//! VirtIO-Block Paravirtualized Storage Driver
//!
//! Controls high-speed VirtIO block devices in virtualized environments (QEMU/KVM),
//! providing Split VirtQueue ring buffers and fast DMA block transfers.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;
use crate::arch::x86_64::io::{inl, inw, outb, outl, outw};
use crate::drivers::pci;
use crate::drivers::virtio::*;
use crate::fs::block::{register_block_device, BlockDevice};
use crate::mm::pmm;
use crate::lunix_println;

pub const VIRTIO_BLK_T_IN: u32 = 0;   // Read
pub const VIRTIO_BLK_T_OUT: u32 = 1;  // Write

pub const VIRTIO_BLK_S_OK: u8 = 0;
pub const VIRTIO_BLK_S_IOERR: u8 = 1;

const QUEUE_SIZE: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VirtioBlkReq {
    pub req_type: u32,
    pub reserved: u32,
    pub sector: u64,
}

pub struct VirtioBlkState {
    avail_idx: u16,
    last_used_idx: u16,
}

pub struct VirtioBlkDevice {
    pub name: String,
    pub io_base: u16,
    pub total_sectors: u64,
    queue_phys: u64,
    state: Mutex<VirtioBlkState>,
}

pub static VIRTIO_BLK_INITIALIZED: AtomicBool = AtomicBool::new(false);
pub static VIRTIO_BLK_DEVICES: Mutex<Vec<Arc<VirtioBlkDevice>>> = Mutex::new(Vec::new());

impl VirtioBlkDevice {
    fn read_sectors_internal(&self, start_sector: u64, count: usize, buf: &mut [u8]) -> Result<(), &'static str> {
        let byte_len = count * 512;
        if buf.len() < byte_len {
            return Err("Destination buffer too small");
        }

        let req_frame = pmm::alloc_frame().ok_or("Failed to allocate VirtIO request frame")?;
        let req_phys = req_frame.as_u64();

        let data_frame = pmm::alloc_frame().ok_or("Failed to allocate VirtIO data frame")?;
        let data_phys = data_frame.as_u64();

        let status_phys = req_phys + 512;

        let mut state = self.state.lock();

        unsafe {
            let req_ptr = req_phys as *mut VirtioBlkReq;
            core::ptr::write_volatile(req_ptr, VirtioBlkReq {
                req_type: VIRTIO_BLK_T_IN,
                reserved: 0,
                sector: start_sector,
            });

            let status_ptr = status_phys as *mut u8;
            core::ptr::write_volatile(status_ptr, 0xFF);

            let desc_table = self.queue_phys as *mut VirtqDesc;

            // Desc 0: Header (Read-only for device)
            *desc_table.add(0) = VirtqDesc {
                addr: req_phys,
                len: core::mem::size_of::<VirtioBlkReq>() as u32,
                flags: VRING_DESC_F_NEXT,
                next: 1,
            };

            // Desc 1: Data Buffer (Write-only for device)
            *desc_table.add(1) = VirtqDesc {
                addr: data_phys,
                len: byte_len as u32,
                flags: VRING_DESC_F_NEXT | VRING_DESC_F_WRITE,
                next: 2,
            };

            // Desc 2: Status Byte (Write-only for device)
            *desc_table.add(2) = VirtqDesc {
                addr: status_phys,
                len: 1,
                flags: VRING_DESC_F_WRITE,
                next: 0,
            };

            // Add descriptor 0 to Available Ring
            let avail_offset = (QUEUE_SIZE * core::mem::size_of::<VirtqDesc>()) as u64;
            let avail_ring_ptr = (self.queue_phys + avail_offset) as *mut u16;

            let cur_idx = state.avail_idx;
            *avail_ring_ptr.add(2 + (cur_idx as usize % QUEUE_SIZE)) = 0;
            state.avail_idx = state.avail_idx.wrapping_add(1);
            core::ptr::write_volatile(avail_ring_ptr.add(1), state.avail_idx);

            // Notify device
            outw(self.io_base + VIRTIO_PCI_QUEUE_NOTIFY, 0);

            // Poll Used Ring
            let used_offset = ((avail_offset + (4 + 2 * QUEUE_SIZE as u64) + 4095) & !4095) as u64;
            let used_ring_ptr = (self.queue_phys + used_offset) as *const u16;

            let mut timeout = 100_000;
            while core::ptr::read_volatile(used_ring_ptr.add(1)) == state.last_used_idx {
                timeout -= 1;
                if timeout == 0 {
                    return Err("VirtIO-Block read timeout");
                }
                core::hint::spin_loop();
            }

            state.last_used_idx = core::ptr::read_volatile(used_ring_ptr.add(1));

            if core::ptr::read_volatile(status_ptr) != VIRTIO_BLK_S_OK {
                return Err("VirtIO-Block device returned error status");
            }

            core::ptr::copy_nonoverlapping(data_phys as *const u8, buf.as_mut_ptr(), byte_len);
        }

        Ok(())
    }

    fn write_sectors_internal(&self, start_sector: u64, count: usize, buf: &[u8]) -> Result<(), &'static str> {
        let byte_len = count * 512;
        if buf.len() < byte_len {
            return Err("Source buffer too small");
        }

        let req_frame = pmm::alloc_frame().ok_or("Failed to allocate VirtIO request frame")?;
        let req_phys = req_frame.as_u64();

        let data_frame = pmm::alloc_frame().ok_or("Failed to allocate VirtIO data frame")?;
        let data_phys = data_frame.as_u64();

        let status_phys = req_phys + 512;

        let mut state = self.state.lock();

        unsafe {
            let req_ptr = req_phys as *mut VirtioBlkReq;
            core::ptr::write_volatile(req_ptr, VirtioBlkReq {
                req_type: VIRTIO_BLK_T_OUT,
                reserved: 0,
                sector: start_sector,
            });

            core::ptr::copy_nonoverlapping(buf.as_ptr(), data_phys as *mut u8, byte_len);

            let status_ptr = status_phys as *mut u8;
            core::ptr::write_volatile(status_ptr, 0xFF);

            let desc_table = self.queue_phys as *mut VirtqDesc;

            // Desc 0: Header (Read-only)
            *desc_table.add(0) = VirtqDesc {
                addr: req_phys,
                len: core::mem::size_of::<VirtioBlkReq>() as u32,
                flags: VRING_DESC_F_NEXT,
                next: 1,
            };

            // Desc 1: Data Buffer (Read-only)
            *desc_table.add(1) = VirtqDesc {
                addr: data_phys,
                len: byte_len as u32,
                flags: VRING_DESC_F_NEXT,
                next: 2,
            };

            // Desc 2: Status Byte (Write-only)
            *desc_table.add(2) = VirtqDesc {
                addr: status_phys,
                len: 1,
                flags: VRING_DESC_F_WRITE,
                next: 0,
            };

            let avail_offset = (QUEUE_SIZE * core::mem::size_of::<VirtqDesc>()) as u64;
            let avail_ring_ptr = (self.queue_phys + avail_offset) as *mut u16;

            let cur_idx = state.avail_idx;
            *avail_ring_ptr.add(2 + (cur_idx as usize % QUEUE_SIZE)) = 0;
            state.avail_idx = state.avail_idx.wrapping_add(1);
            core::ptr::write_volatile(avail_ring_ptr.add(1), state.avail_idx);

            outw(self.io_base + VIRTIO_PCI_QUEUE_NOTIFY, 0);

            let used_offset = ((avail_offset + (4 + 2 * QUEUE_SIZE as u64) + 4095) & !4095) as u64;
            let used_ring_ptr = (self.queue_phys + used_offset) as *const u16;

            let mut timeout = 100_000;
            while core::ptr::read_volatile(used_ring_ptr.add(1)) == state.last_used_idx {
                timeout -= 1;
                if timeout == 0 {
                    return Err("VirtIO-Block write timeout");
                }
                core::hint::spin_loop();
            }

            state.last_used_idx = core::ptr::read_volatile(used_ring_ptr.add(1));

            if core::ptr::read_volatile(status_ptr) != VIRTIO_BLK_S_OK {
                return Err("VirtIO-Block device returned error status");
            }
        }

        Ok(())
    }
}

impl BlockDevice for VirtioBlkDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn total_blocks(&self) -> u64 {
        self.total_sectors
    }

    fn read_blocks(&self, start_block: u64, count: usize, buf: &mut [u8]) -> Result<(), &'static str> {
        self.read_sectors_internal(start_block, count, buf)
    }

    fn write_blocks(&self, start_block: u64, count: usize, buf: &[u8]) -> Result<(), &'static str> {
        self.write_sectors_internal(start_block, count, buf)
    }
}

pub fn init() {
    lunix_println!("[storage] Scanning for VirtIO-Block Virtual Storage Controllers...");

    let pci_dev = match pci::find_device(0x1AF4, 0x1001)
        .or_else(|| pci::find_device(0x1AF4, 0x1042)) {
        Some(dev) => dev,
        None => {
            lunix_serial_println!("  [VirtIO-Blk] No VirtIO block device detected.");
            return;
        }
    };

    pci::enable_bus_mastering(&pci_dev);

    let io_base = (pci_dev.bar0 & 0xFFFC) as u16;
    lunix_serial_println!("  [VirtIO-Blk] Found device at {:X}:{:X}.0, I/O Port: 0x{:X}", pci_dev.bus, pci_dev.device, io_base);

    unsafe {
        // Reset Device
        outb(io_base + VIRTIO_PCI_STATUS, 0);

        // Acknowledge & Driver status
        outb(io_base + VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER);

        // Select Queue 0
        outw(io_base + VIRTIO_PCI_QUEUE_SEL, 0);
        let q_num = inw(io_base + VIRTIO_PCI_QUEUE_NUM);
        if q_num == 0 {
            lunix_serial_println!("  [VirtIO-Blk] Queue 0 not available");
            return;
        }

        // Allocate 2 frames for queue rings
        let q_frame = match pmm::alloc_frame() {
            Some(f) => f.as_u64(),
            None => return,
        };
        let _ = pmm::alloc_frame(); // Second frame for alignment

        core::ptr::write_bytes(q_frame as *mut u8, 0, 8192);

        // Configure PFN
        outl(io_base + VIRTIO_PCI_QUEUE_PFN, (q_frame / 4096) as u32);

        // Read total capacity (in 512B sectors) at config offset 0x14
        let cap_low = inl(io_base + 0x14);
        let cap_high = inl(io_base + 0x18);
        let total_sectors = ((cap_high as u64) << 32) | (cap_low as u64);

        // Driver OK
        outb(io_base + VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK);

        let dev = VirtioBlkDevice {
            name: String::from("vda"),
            io_base,
            total_sectors: if total_sectors > 0 { total_sectors } else { 131072 },
            queue_phys: q_frame,
            state: Mutex::new(VirtioBlkState {
                avail_idx: 0,
                last_used_idx: 0,
            }),
        };

        let dev_arc = Arc::new(dev);
        register_block_device(dev_arc.clone());
        VIRTIO_BLK_DEVICES.lock().push(dev_arc);
        VIRTIO_BLK_INITIALIZED.store(true, Ordering::SeqCst);

        lunix_println!("[+] VirtIO-Block Driver initialized (/dev/vda mounted).");
        lunix_serial_println!("[+] VirtIO-Block Driver initialized (/dev/vda mounted).");
    }
}
