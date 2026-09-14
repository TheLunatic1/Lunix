//! AHCI (Advanced Host Controller Interface) SATA Storage Driver
//!
//! Controls PCI SATA/AHCI controllers on bare-metal x86_64, supporting
//! ABAR memory mapping, Port Command Lists, Received FIS, Physical Region
//! Descriptor Tables (PRDT), ATA IDENTIFY, and high-speed DMA block transfers.

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;
use x86_64::PhysAddr;
use crate::drivers::pci;
use crate::fs::block::{register_block_device, BlockDevice};
use crate::mm::{pmm, vmm};
use crate::lunix_println;

pub const SATA_SIG_ATA: u32 = 0x0000_0101;   // SATA Drive
pub const SATA_SIG_ATAPI: u32 = 0xEB14_0101; // SATAPI Drive
pub const SATA_SIG_SEMB: u32 = 0xC33C_0101;  // Enclosure Management Bridge
pub const SATA_SIG_PM: u32 = 0x9669_0101;    // Port Multiplier

const HBA_PORT_CMD_ST: u32 = 1 << 0;
const HBA_PORT_CMD_FRE: u32 = 1 << 4;
const HBA_PORT_CMD_FR: u32 = 1 << 14;
const HBA_PORT_CMD_CR: u32 = 1 << 15;

const HBA_PXIS_TFES: u32 = 1 << 30;

const ATA_CMD_READ_DMA_EXT: u8 = 0x25;
const ATA_CMD_WRITE_DMA_EXT: u8 = 0x35;
const ATA_CMD_IDENTIFY: u8 = 0xEC;

const FIS_TYPE_REG_H2D: u8 = 0x27;

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct HbaPrdtEntry {
    pub dba: u32,
    pub dbau: u32,
    pub rsv0: u32,
    pub dbc: u32, // bit 0..21: byte count - 1, bit 31: interrupt on completion
}

#[repr(C, packed)]
pub struct HbaCmdTable {
    pub cfis: [u8; 64],
    pub acmd: [u8; 16],
    pub rsv: [u8; 48],
    pub prdt_entry: [HbaPrdtEntry; 1],
}

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct HbaCmdHeader {
    pub flags: u16, // bit 0..4: CFL, bit 5: ATAPI, bit 6: Write, bit 7: Prefetchable
    pub prdtl: u16, // PRDT length
    pub prdbc: u32, // PRD byte count transferred
    pub ctba: u32,  // Command Table base address (lower)
    pub ctbau: u32, // Command Table base address (upper)
    pub rsv1: [u32; 4],
}

#[repr(C)]
pub struct HbaPort {
    pub clb: u32,
    pub clbu: u32,
    pub fb: u32,
    pub fbu: u32,
    pub is: u32,
    pub ie: u32,
    pub cmd: u32,
    pub rsv0: u32,
    pub tfd: u32,
    pub sig: u32,
    pub ssts: u32,
    pub sctl: u32,
    pub serr: u32,
    pub sact: u32,
    pub ci: u32,
    pub sntf: u32,
    pub fbs: u32,
    pub rsv1: [u32; 11],
    pub vendor: [u32; 4],
}

#[repr(C)]
pub struct HbaMem {
    pub cap: u32,
    pub ghc: u32,
    pub is: u32,
    pub pi: u32,
    pub vs: u32,
    pub ccc_ctl: u32,
    pub ccc_pts: u32,
    pub em_loc: u32,
    pub em_ctl: u32,
    pub cap2: u32,
    pub bohc: u32,
    pub rsv: [u8; 0xA0 - 0x2C],
    pub vendor: [u8; 0x100 - 0xA0],
    pub ports: [HbaPort; 32],
}

pub struct AhciDevice {
    pub name: String,
    pub port_idx: usize,
    pub abar_base: u64,
    pub total_sectors: u64,
    cmd_header_phys: u64,
    cmd_table_phys: u64,
    lock: Mutex<()>,
}

pub static AHCI_INITIALIZED: AtomicBool = AtomicBool::new(false);
pub static AHCI_DEVICES: Mutex<Vec<Arc<AhciDevice>>> = Mutex::new(Vec::new());

impl AhciDevice {
    fn get_hba_mem(&self) -> &mut HbaMem {
        unsafe { &mut *(self.abar_base as *mut HbaMem) }
    }

    fn get_port(&self) -> &mut HbaPort {
        let hba = self.get_hba_mem();
        &mut hba.ports[self.port_idx]
    }

    fn start_port(&self) {
        let port = self.get_port();
        unsafe {
            while (core::ptr::read_volatile(&port.cmd) & HBA_PORT_CMD_CR) != 0 {
                core::hint::spin_loop();
            }
            let cmd = core::ptr::read_volatile(&port.cmd) | HBA_PORT_CMD_FRE | HBA_PORT_CMD_ST;
            core::ptr::write_volatile(&mut port.cmd, cmd);
        }
    }

    fn stop_port(&self) {
        let port = self.get_port();
        unsafe {
            let mut cmd = core::ptr::read_volatile(&port.cmd);
            cmd &= !HBA_PORT_CMD_ST;
            cmd &= !HBA_PORT_CMD_FRE;
            core::ptr::write_volatile(&mut port.cmd, cmd);

            while (core::ptr::read_volatile(&port.cmd) & (HBA_PORT_CMD_FR | HBA_PORT_CMD_CR)) != 0 {
                core::hint::spin_loop();
            }
        }
    }

    pub fn identify(&mut self) -> Result<(), &'static str> {
        let frame = pmm::alloc_frame().ok_or("Failed to allocate IDENTIFY DMA frame")?;
        let buf_phys = frame.as_u64();
        let buf_ptr = buf_phys as *mut u16;

        unsafe {
            core::ptr::write_bytes(buf_ptr as *mut u8, 0, 512);

            let cmd_table = &mut *(self.cmd_table_phys as *mut HbaCmdTable);
            core::ptr::write_bytes(cmd_table as *mut _ as *mut u8, 0, core::mem::size_of::<HbaCmdTable>());

            cmd_table.prdt_entry[0].dba = buf_phys as u32;
            cmd_table.prdt_entry[0].dbau = (buf_phys >> 32) as u32;
            cmd_table.prdt_entry[0].dbc = 512 - 1; // 512 bytes

            // Setup Command FIS (Register H2D)
            cmd_table.cfis[0] = FIS_TYPE_REG_H2D;
            cmd_table.cfis[1] = 1 << 7; // Command bit
            cmd_table.cfis[2] = ATA_CMD_IDENTIFY;

            let cmd_header = &mut *(self.cmd_header_phys as *mut HbaCmdHeader);
            cmd_header.flags = 5 & 0x1F; // 5 Dwords FIS size
            cmd_header.prdtl = 1;
            cmd_header.prdbc = 0;
            cmd_header.ctba = self.cmd_table_phys as u32;
            cmd_header.ctbau = (self.cmd_table_phys >> 32) as u32;

            let port = self.get_port();
            core::ptr::write_volatile(&mut port.ci, 1); // Issue command on slot 0

            // Wait for completion
            let mut timeout = 100_000;
            while (core::ptr::read_volatile(&port.ci) & 1) != 0 {
                timeout -= 1;
                if timeout == 0 {
                    return Err("AHCI IDENTIFY command timeout");
                }
                core::hint::spin_loop();
            }

            if (core::ptr::read_volatile(&port.is) & HBA_PXIS_TFES) != 0 {
                return Err("AHCI Task File Error during IDENTIFY");
            }

            let lba28_sectors = (*buf_ptr.add(60) as u32) | ((*buf_ptr.add(61) as u32) << 16);
            let lba48_sectors = (*buf_ptr.add(100) as u64)
                | ((*buf_ptr.add(101) as u64) << 16)
                | ((*buf_ptr.add(102) as u64) << 32)
                | ((*buf_ptr.add(103) as u64) << 48);

            self.total_sectors = if lba48_sectors > 0 {
                lba48_sectors
            } else if lba28_sectors > 0 {
                lba28_sectors as u64
            } else {
                131072 // Fallback default 64MB if unspecified
            };
        }

        Ok(())
    }

    pub fn read_sectors_dma(&self, lba: u64, count: usize, buf: &mut [u8]) -> Result<(), &'static str> {
        let byte_len = count * 512;
        if buf.len() < byte_len {
            return Err("Destination buffer too small");
        }

        let frame = pmm::alloc_frame().ok_or("Failed to allocate AHCI DMA transfer frame")?;
        let dma_phys = frame.as_u64();

        unsafe {
            let cmd_table = &mut *(self.cmd_table_phys as *mut HbaCmdTable);
            core::ptr::write_bytes(cmd_table as *mut _ as *mut u8, 0, core::mem::size_of::<HbaCmdTable>());

            cmd_table.prdt_entry[0].dba = dma_phys as u32;
            cmd_table.prdt_entry[0].dbau = (dma_phys >> 32) as u32;
            cmd_table.prdt_entry[0].dbc = (byte_len as u32) - 1;

            // Setup Command FIS (Register H2D) for READ DMA EXT
            cmd_table.cfis[0] = FIS_TYPE_REG_H2D;
            cmd_table.cfis[1] = 1 << 7; // Command bit
            cmd_table.cfis[2] = ATA_CMD_READ_DMA_EXT;

            cmd_table.cfis[4] = (lba & 0xFF) as u8;
            cmd_table.cfis[5] = ((lba >> 8) & 0xFF) as u8;
            cmd_table.cfis[6] = ((lba >> 16) & 0xFF) as u8;
            cmd_table.cfis[7] = 1 << 6; // LBA mode (device bit)

            cmd_table.cfis[8] = ((lba >> 24) & 0xFF) as u8;
            cmd_table.cfis[9] = ((lba >> 32) & 0xFF) as u8;
            cmd_table.cfis[10] = ((lba >> 40) & 0xFF) as u8;

            cmd_table.cfis[12] = (count & 0xFF) as u8;
            cmd_table.cfis[13] = ((count >> 8) & 0xFF) as u8;

            let cmd_header = &mut *(self.cmd_header_phys as *mut HbaCmdHeader);
            cmd_header.flags = 5 & 0x1F; // 5 Dwords FIS size, Read
            cmd_header.prdtl = 1;
            cmd_header.prdbc = 0;
            cmd_header.ctba = self.cmd_table_phys as u32;
            cmd_header.ctbau = (self.cmd_table_phys >> 32) as u32;

            let port = self.get_port();
            core::ptr::write_volatile(&mut port.ci, 1);

            let mut timeout = 100_000;
            while (core::ptr::read_volatile(&port.ci) & 1) != 0 {
                timeout -= 1;
                if timeout == 0 {
                    return Err("AHCI READ DMA EXT command timeout");
                }
                core::hint::spin_loop();
            }

            if (core::ptr::read_volatile(&port.is) & HBA_PXIS_TFES) != 0 {
                return Err("AHCI Task File Error during READ");
            }

            // Copy data to destination buffer
            core::ptr::copy_nonoverlapping(dma_phys as *const u8, buf.as_mut_ptr(), byte_len);
        }

        Ok(())
    }

    pub fn write_sectors_dma(&self, lba: u64, count: usize, buf: &[u8]) -> Result<(), &'static str> {
        let byte_len = count * 512;
        if buf.len() < byte_len {
            return Err("Source buffer too small");
        }

        let frame = pmm::alloc_frame().ok_or("Failed to allocate AHCI DMA transfer frame")?;
        let dma_phys = frame.as_u64();

        unsafe {
            core::ptr::copy_nonoverlapping(buf.as_ptr(), dma_phys as *mut u8, byte_len);

            let cmd_table = &mut *(self.cmd_table_phys as *mut HbaCmdTable);
            core::ptr::write_bytes(cmd_table as *mut _ as *mut u8, 0, core::mem::size_of::<HbaCmdTable>());

            cmd_table.prdt_entry[0].dba = dma_phys as u32;
            cmd_table.prdt_entry[0].dbau = (dma_phys >> 32) as u32;
            cmd_table.prdt_entry[0].dbc = (byte_len as u32) - 1;

            // Setup Command FIS (Register H2D) for WRITE DMA EXT
            cmd_table.cfis[0] = FIS_TYPE_REG_H2D;
            cmd_table.cfis[1] = 1 << 7; // Command bit
            cmd_table.cfis[2] = ATA_CMD_WRITE_DMA_EXT;

            cmd_table.cfis[4] = (lba & 0xFF) as u8;
            cmd_table.cfis[5] = ((lba >> 8) & 0xFF) as u8;
            cmd_table.cfis[6] = ((lba >> 16) & 0xFF) as u8;
            cmd_table.cfis[7] = 1 << 6; // LBA mode

            cmd_table.cfis[8] = ((lba >> 24) & 0xFF) as u8;
            cmd_table.cfis[9] = ((lba >> 32) & 0xFF) as u8;
            cmd_table.cfis[10] = ((lba >> 40) & 0xFF) as u8;

            cmd_table.cfis[12] = (count & 0xFF) as u8;
            cmd_table.cfis[13] = ((count >> 8) & 0xFF) as u8;

            let cmd_header = &mut *(self.cmd_header_phys as *mut HbaCmdHeader);
            cmd_header.flags = (5 & 0x1F) | (1 << 6); // 5 Dwords FIS size, Write flag set
            cmd_header.prdtl = 1;
            cmd_header.prdbc = 0;
            cmd_header.ctba = self.cmd_table_phys as u32;
            cmd_header.ctbau = (self.cmd_table_phys >> 32) as u32;

            let port = self.get_port();
            core::ptr::write_volatile(&mut port.ci, 1);

            let mut timeout = 100_000;
            while (core::ptr::read_volatile(&port.ci) & 1) != 0 {
                timeout -= 1;
                if timeout == 0 {
                    return Err("AHCI WRITE DMA EXT command timeout");
                }
                core::hint::spin_loop();
            }

            if (core::ptr::read_volatile(&port.is) & HBA_PXIS_TFES) != 0 {
                return Err("AHCI Task File Error during WRITE");
            }
        }

        Ok(())
    }
}

impl BlockDevice for AhciDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn total_blocks(&self) -> u64 {
        self.total_sectors
    }

    fn read_blocks(&self, start_block: u64, count: usize, buf: &mut [u8]) -> Result<(), &'static str> {
        let _guard = self.lock.lock();
        self.read_sectors_dma(start_block, count, buf)
    }

    fn write_blocks(&self, start_block: u64, count: usize, buf: &[u8]) -> Result<(), &'static str> {
        let _guard = self.lock.lock();
        self.write_sectors_dma(start_block, count, buf)
    }
}

pub fn init() {
    lunix_println!("[storage] Scanning for AHCI / SATA Host Controllers...");

    let pci_dev = match pci::find_by_class(0x01, 0x06)
        .or_else(|| pci::find_device(0x8086, 0x2922))
        .or_else(|| pci::find_device(0x8086, 0x2829))
        .or_else(|| pci::find_device(0x1B36, 0x0002)) {
        Some(dev) => dev,
        None => {
            lunix_serial_println!("  [AHCI] No PCI AHCI / SATA controller detected.");
            return;
        }
    };

    pci::enable_bus_mastering(&pci_dev);

    let abar_phys = (pci_dev.bar0 & 0xFFFF_FFF0) as u64; // In some controllers BAR5, in others BAR0
    let bar5_raw = pci::pci_read_u32(pci_dev.bus, pci_dev.device, pci_dev.function, 0x24);
    let abar_effective = if (bar5_raw & 0xFFFF_FFF0) != 0 {
        (bar5_raw & 0xFFFF_FFF0) as u64
    } else {
        abar_phys
    };

    let abar_virt = match vmm::map_mmio_range(PhysAddr::new(abar_effective), 4096 * 4) {
        Ok(v) => v.as_u64(),
        Err(_) => {
            lunix_serial_println!("  [AHCI] Failed to map ABAR memory range");
            return;
        }
    };

    lunix_serial_println!("  [AHCI] Found Controller at {:X}:{:X}.0, ABAR=0x{:X}", pci_dev.bus, pci_dev.device, abar_effective);

    let hba = unsafe { &mut *(abar_virt as *mut HbaMem) };

    // Enable AHCI mode
    unsafe {
        core::ptr::write_volatile(&mut hba.ghc, core::ptr::read_volatile(&hba.ghc) | (1 << 31));
    }

    let pi = unsafe { core::ptr::read_volatile(&hba.pi) };
    let mut discovered_drives = 0;

    for i in 0..32 {
        if (pi & (1 << i)) != 0 {
            let port = &mut hba.ports[i];
            let ssts = unsafe { core::ptr::read_volatile(&port.ssts) };
            let det = ssts & 0x0F;
            let ipm = (ssts >> 8) & 0x0F;

            if det == 3 && ipm == 1 {
                let sig = unsafe { core::ptr::read_volatile(&port.sig) };
                if sig == SATA_SIG_ATA {
                    lunix_serial_println!("  [AHCI] SATA Drive detected on Port {}", i);

                    // Allocate Command List & FIS frames
                    let cl_frame = match pmm::alloc_frame() {
                        Some(f) => f.as_u64(),
                        None => continue,
                    };
                    let fb_frame = match pmm::alloc_frame() {
                        Some(f) => f.as_u64(),
                        None => continue,
                    };
                    let ct_frame = match pmm::alloc_frame() {
                        Some(f) => f.as_u64(),
                        None => continue,
                    };

                    unsafe {
                        core::ptr::write_bytes(cl_frame as *mut u8, 0, 4096);
                        core::ptr::write_bytes(fb_frame as *mut u8, 0, 4096);
                        core::ptr::write_bytes(ct_frame as *mut u8, 0, 4096);

                        core::ptr::write_volatile(&mut port.clb, cl_frame as u32);
                        core::ptr::write_volatile(&mut port.clbu, (cl_frame >> 32) as u32);
                        core::ptr::write_volatile(&mut port.fb, fb_frame as u32);
                        core::ptr::write_volatile(&mut port.fbu, (fb_frame >> 32) as u32);
                    }

                    let dev_name = format!("ahci{}", discovered_drives);
                    let mut ahci_dev = AhciDevice {
                        name: dev_name,
                        port_idx: i,
                        abar_base: abar_virt,
                        total_sectors: 131072,
                        cmd_header_phys: cl_frame,
                        cmd_table_phys: ct_frame,
                        lock: Mutex::new(()),
                    };

                    ahci_dev.start_port();
                    let _ = ahci_dev.identify();

                    let dev_arc = Arc::new(ahci_dev);
                    register_block_device(dev_arc.clone());
                    AHCI_DEVICES.lock().push(dev_arc);
                    discovered_drives += 1;
                }
            }
        }
    }

    if discovered_drives > 0 {
        AHCI_INITIALIZED.store(true, Ordering::SeqCst);
        lunix_println!("[+] AHCI SATA Driver initialized ({} drive(s) mounted).", discovered_drives);
    }
}
