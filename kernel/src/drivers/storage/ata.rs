use alloc::string::String;
use alloc::sync::Arc;
use crate::arch::x86_64::io::{inb, inw, outb, outw};
use crate::fs::block::{register_block_device, BlockDevice};
use spin::Mutex;

const ATA_STATUS_BSY: u8 = 0x80;
const ATA_STATUS_DRDY: u8 = 0x40;
const ATA_STATUS_DRQ: u8 = 0x08;
const ATA_STATUS_ERR: u8 = 0x01;

const ATA_CMD_READ_SECTORS: u8 = 0x20;
const ATA_CMD_WRITE_SECTORS: u8 = 0x30;
const ATA_CMD_IDENTIFY: u8 = 0xEC;

pub struct AtaDevice {
    pub name: String,
    pub io_base: u16,
    pub ctrl_base: u16,
    pub is_slave: bool,
    pub total_sectors: u64,
    lock: Mutex<()>,
}

impl AtaDevice {
    pub fn new(name: &str, io_base: u16, ctrl_base: u16, is_slave: bool) -> Option<Self> {
        let mut dev = Self {
            name: String::from(name),
            io_base,
            ctrl_base,
            is_slave,
            total_sectors: 0,
            lock: Mutex::new(()),
        };

        if dev.identify() {
            Some(dev)
        } else {
            None
        }
    }

    fn wait_bsy(&self) -> Result<(), &'static str> {
        for _ in 0..100_000 {
            let status = unsafe { inb(self.io_base + 7) };
            if (status & ATA_STATUS_BSY) == 0 {
                return Ok(());
            }
        }
        Err("ATA Timeout waiting for BSY to clear")
    }

    fn wait_drq(&self) -> Result<(), &'static str> {
        for _ in 0..100_000 {
            let status = unsafe { inb(self.io_base + 7) };
            if (status & ATA_STATUS_ERR) != 0 {
                return Err("ATA Error status set");
            }
            if (status & ATA_STATUS_DRQ) != 0 {
                return Ok(());
            }
        }
        Err("ATA Timeout waiting for DRQ")
    }

    fn identify(&mut self) -> bool {
        unsafe {
            // Select drive: Master = 0xA0, Slave = 0xB0
            outb(self.io_base + 6, if self.is_slave { 0xB0 } else { 0xA0 });
            outb(self.io_base + 2, 0);
            outb(self.io_base + 3, 0);
            outb(self.io_base + 4, 0);
            outb(self.io_base + 5, 0);
            outb(self.io_base + 7, ATA_CMD_IDENTIFY);

            let status = inb(self.io_base + 7);
            if status == 0 || status == 0xFF {
                return false; // No device present
            }

            if self.wait_bsy().is_err() {
                return false;
            }

            // Check if non-ATA device (ATAPI)
            let mid = inb(self.io_base + 4);
            let high = inb(self.io_base + 5);
            if mid != 0 || high != 0 {
                return false; // ATAPI device
            }

            if self.wait_drq().is_err() {
                return false;
            }

            // Read 256 words of IDENTIFY data
            let mut identify_buf = [0u16; 256];
            for i in 0..256 {
                identify_buf[i] = inw(self.io_base);
            }

            // Sectors (LBA28 is at words 60-61, LBA48 at words 100-103)
            let lba28_sectors = (identify_buf[60] as u32) | ((identify_buf[61] as u32) << 16);
            let lba48_sectors = (identify_buf[100] as u64)
                | ((identify_buf[101] as u64) << 16)
                | ((identify_buf[102] as u64) << 32)
                | ((identify_buf[103] as u64) << 48);

            self.total_sectors = if lba48_sectors > 0 {
                lba48_sectors
            } else {
                lba28_sectors as u64
            };

            true
        }
    }

    fn read_sector_internal(&self, lba: u64, buf: &mut [u8]) -> Result<(), &'static str> {
        if buf.len() < 512 {
            return Err("Buffer too small for 512-byte sector");
        }

        unsafe {
            self.wait_bsy()?;

            // Send Drive / LBA bits 24..27
            let drive_head = 0xE0 | (if self.is_slave { 0x10 } else { 0 }) | (((lba >> 24) & 0x0F) as u8);
            outb(self.io_base + 6, drive_head);

            outb(self.io_base + 2, 1); // 1 sector
            outb(self.io_base + 3, (lba & 0xFF) as u8);
            outb(self.io_base + 4, ((lba >> 8) & 0xFF) as u8);
            outb(self.io_base + 5, ((lba >> 16) & 0xFF) as u8);
            outb(self.io_base + 7, ATA_CMD_READ_SECTORS);

            self.wait_bsy()?;
            self.wait_drq()?;

            // Read 256 words (512 bytes)
            let word_buf = buf.as_mut_ptr() as *mut u16;
            for i in 0..256 {
                let w = inw(self.io_base);
                core::ptr::write_unaligned(word_buf.add(i), w);
            }

            Ok(())
        }
    }

    fn write_sector_internal(&self, lba: u64, buf: &[u8]) -> Result<(), &'static str> {
        if buf.len() < 512 {
            return Err("Buffer too small for 512-byte sector");
        }

        unsafe {
            self.wait_bsy()?;

            let drive_head = 0xE0 | (if self.is_slave { 0x10 } else { 0 }) | (((lba >> 24) & 0x0F) as u8);
            outb(self.io_base + 6, drive_head);

            outb(self.io_base + 2, 1);
            outb(self.io_base + 3, (lba & 0xFF) as u8);
            outb(self.io_base + 4, ((lba >> 8) & 0xFF) as u8);
            outb(self.io_base + 5, ((lba >> 16) & 0xFF) as u8);
            outb(self.io_base + 7, ATA_CMD_WRITE_SECTORS);

            self.wait_bsy()?;
            self.wait_drq()?;

            // Write 256 words (512 bytes)
            let word_buf = buf.as_ptr() as *const u16;
            for i in 0..256 {
                let w = core::ptr::read_unaligned(word_buf.add(i));
                outw(self.io_base, w);
            }

            // Flush cache
            outb(self.io_base + 7, 0xE7);
            self.wait_bsy()?;

            Ok(())
        }
    }
}

impl BlockDevice for AtaDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn total_blocks(&self) -> u64 {
        self.total_sectors
    }

    fn read_blocks(&self, start_block: u64, count: usize, buf: &mut [u8]) -> Result<(), &'static str> {
        let _guard = self.lock.lock();
        if buf.len() < count * 512 {
            return Err("Destination buffer too small");
        }

        for i in 0..count {
            let offset = i * 512;
            let sector_slice = &mut buf[offset..offset + 512];
            self.read_sector_internal(start_block + i as u64, sector_slice)?;
        }
        Ok(())
    }

    fn write_blocks(&self, start_block: u64, count: usize, buf: &[u8]) -> Result<(), &'static str> {
        let _guard = self.lock.lock();
        if buf.len() < count * 512 {
            return Err("Source buffer too small");
        }

        for i in 0..count {
            let offset = i * 512;
            let sector_slice = &buf[offset..offset + 512];
            self.write_sector_internal(start_block + i as u64, sector_slice)?;
        }
        Ok(())
    }
}

pub fn init() {
    lunix_serial_println!("[storage] Scanning ATA / IDE Storage Controllers...");

    // Primary Master (sda)
    if let Some(dev) = AtaDevice::new("sda", 0x1F0, 0x3F6, false) {
        register_block_device(Arc::new(dev));
    }

    // Primary Slave (sdb)
    if let Some(dev) = AtaDevice::new("sdb", 0x1F0, 0x3F6, true) {
        register_block_device(Arc::new(dev));
    }

    // Secondary Master (sdc)
    if let Some(dev) = AtaDevice::new("sdc", 0x170, 0x376, false) {
        register_block_device(Arc::new(dev));
    }
}
