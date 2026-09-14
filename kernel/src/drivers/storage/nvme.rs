//! NVMe (NVM Express) High-Performance PCIe Storage Driver
//!
//! Controls PCI Express NVMe solid-state storage controllers on bare-metal x86_64,
//! supporting Admin Queue Pairs, Controller Enable/Ready handshakes, Namespace Identification,
//! I/O Submission/Completion Queue allocation, and high-speed PRP DMA block transfers.

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

const NVME_ADMIN_IDENTIFY: u8 = 0x06;
const NVME_ADMIN_CREATE_IO_CQ: u8 = 0x05;
const NVME_ADMIN_CREATE_IO_SQ: u8 = 0x01;

const NVME_NVM_CMD_WRITE: u8 = 0x01;
const NVME_NVM_CMD_READ: u8 = 0x02;

const QUEUE_SIZE: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct NvmeCmd {
    pub cdw0: u32,
    pub nsid: u32,
    pub rsvd2: u64,
    pub mptr: u64,
    pub prp1: u64,
    pub prp2: u64,
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct NvmeCqe {
    pub dw0: u32,
    pub dw1: u32,
    pub sq_hd: u16,
    pub sq_id: u16,
    pub cid: u16,
    pub status: u16,
}

pub struct NvmeQueues {
    admin_sq_tail: usize,
    admin_cq_head: usize,
    admin_cq_phase: u8,
    io_sq_tail: usize,
    io_cq_head: usize,
    io_cq_phase: u8,
}

pub struct NvmeDevice {
    pub name: String,
    pub bar0_base: u64,
    pub total_sectors: u64,
    pub sector_size: usize,
    admin_sq_phys: u64,
    admin_cq_phys: u64,
    io_sq_phys: u64,
    io_cq_phys: u64,
    db_stride: usize,
    queues: Mutex<NvmeQueues>,
}

pub static NVME_INITIALIZED: AtomicBool = AtomicBool::new(false);
pub static NVME_DEVICES: Mutex<Vec<Arc<NvmeDevice>>> = Mutex::new(Vec::new());

impl NvmeDevice {
    #[inline(always)]
    fn read_reg32(&self, offset: usize) -> u32 {
        unsafe {
            let ptr = (self.bar0_base + offset as u64) as *const u32;
            core::ptr::read_volatile(ptr)
        }
    }

    #[inline(always)]
    fn write_reg32(&self, offset: usize, value: u32) {
        unsafe {
            let ptr = (self.bar0_base + offset as u64) as *mut u32;
            core::ptr::write_volatile(ptr, value);
        }
    }

    #[inline(always)]
    fn write_reg64(&self, offset: usize, value: u64) {
        unsafe {
            let ptr = (self.bar0_base + offset as u64) as *mut u64;
            core::ptr::write_volatile(ptr, value);
        }
    }

    fn ring_admin_sq_doorbell(&self, tail: u32) {
        self.write_reg32(0x1000, tail);
    }

    fn ring_admin_cq_doorbell(&self, head: u32) {
        let offset = 0x1000 + (1 << (2 + self.db_stride));
        self.write_reg32(offset, head);
    }

    fn ring_io_sq_doorbell(&self, qid: usize, tail: u32) {
        let offset = 0x1000 + (2 * qid << (2 + self.db_stride));
        self.write_reg32(offset, tail);
    }

    fn ring_io_cq_doorbell(&self, qid: usize, head: u32) {
        let offset = 0x1000 + (((2 * qid) + 1) << (2 + self.db_stride));
        self.write_reg32(offset, head);
    }

    fn submit_admin_cmd(&self, cmd: NvmeCmd) -> Result<NvmeCqe, &'static str> {
        let mut q = self.queues.lock();
        let sq_ptr = (self.admin_sq_phys as *mut NvmeCmd).wrapping_add(q.admin_sq_tail);
        unsafe {
            core::ptr::write_volatile(sq_ptr, cmd);
        }

        q.admin_sq_tail = (q.admin_sq_tail + 1) % QUEUE_SIZE;
        self.ring_admin_sq_doorbell(q.admin_sq_tail as u32);

        // Wait for CQE
        let cq_ptr = (self.admin_cq_phys as *const NvmeCqe).wrapping_add(q.admin_cq_head);
        let mut timeout = 100_000;
        loop {
            let cqe = unsafe { core::ptr::read_volatile(cq_ptr) };
            let phase = ((cqe.status & 1) != 0) as u8;
            if phase == q.admin_cq_phase {
                q.admin_cq_head = (q.admin_cq_head + 1) % QUEUE_SIZE;
                if q.admin_cq_head == 0 {
                    q.admin_cq_phase ^= 1;
                }
                self.ring_admin_cq_doorbell(q.admin_cq_head as u32);

                if (cqe.status >> 1) != 0 {
                    return Err("NVMe Admin Command returned non-zero status");
                }
                return Ok(cqe);
            }
            timeout -= 1;
            if timeout == 0 {
                return Err("NVMe Admin Command timed out");
            }
            core::hint::spin_loop();
        }
    }

    pub fn read_blocks_dma(&self, start_block: u64, count: usize, buf: &mut [u8]) -> Result<(), &'static str> {
        let byte_len = count * self.sector_size;
        if buf.len() < byte_len {
            return Err("Destination buffer too small");
        }

        let dma_frame = pmm::alloc_frame().ok_or("Failed to allocate NVMe DMA frame")?;
        let dma_phys = dma_frame.as_u64();

        let mut q = self.queues.lock();

        let mut cmd = NvmeCmd::default();
        cmd.cdw0 = (NVME_NVM_CMD_READ as u32) | ((q.io_sq_tail as u32) << 16);
        cmd.nsid = 1;
        cmd.prp1 = dma_phys;
        cmd.cdw10 = (start_block & 0xFFFF_FFFF) as u32;
        cmd.cdw11 = ((start_block >> 32) & 0xFFFF_FFFF) as u32;
        cmd.cdw12 = (count as u32) - 1; // 0-based count

        let sq_ptr = (self.io_sq_phys as *mut NvmeCmd).wrapping_add(q.io_sq_tail);
        unsafe {
            core::ptr::write_volatile(sq_ptr, cmd);
        }

        q.io_sq_tail = (q.io_sq_tail + 1) % QUEUE_SIZE;
        self.ring_io_sq_doorbell(1, q.io_sq_tail as u32);

        // Wait on I/O CQ
        let cq_ptr = (self.io_cq_phys as *const NvmeCqe).wrapping_add(q.io_cq_head);
        let mut timeout = 100_000;
        loop {
            let cqe = unsafe { core::ptr::read_volatile(cq_ptr) };
            let phase = ((cqe.status & 1) != 0) as u8;
            if phase == q.io_cq_phase {
                q.io_cq_head = (q.io_cq_head + 1) % QUEUE_SIZE;
                if q.io_cq_head == 0 {
                    q.io_cq_phase ^= 1;
                }
                self.ring_io_cq_doorbell(1, q.io_cq_head as u32);

                if (cqe.status >> 1) != 0 {
                    return Err("NVMe Read Command failed");
                }
                break;
            }
            timeout -= 1;
            if timeout == 0 {
                return Err("NVMe Read Command timed out");
            }
            core::hint::spin_loop();
        }

        unsafe {
            core::ptr::copy_nonoverlapping(dma_phys as *const u8, buf.as_mut_ptr(), byte_len);
        }

        Ok(())
    }

    pub fn write_blocks_dma(&self, start_block: u64, count: usize, buf: &[u8]) -> Result<(), &'static str> {
        let byte_len = count * self.sector_size;
        if buf.len() < byte_len {
            return Err("Source buffer too small");
        }

        let dma_frame = pmm::alloc_frame().ok_or("Failed to allocate NVMe DMA frame")?;
        let dma_phys = dma_frame.as_u64();

        unsafe {
            core::ptr::copy_nonoverlapping(buf.as_ptr(), dma_phys as *mut u8, byte_len);
        }

        let mut q = self.queues.lock();

        let mut cmd = NvmeCmd::default();
        cmd.cdw0 = (NVME_NVM_CMD_WRITE as u32) | ((q.io_sq_tail as u32) << 16);
        cmd.nsid = 1;
        cmd.prp1 = dma_phys;
        cmd.cdw10 = (start_block & 0xFFFF_FFFF) as u32;
        cmd.cdw11 = ((start_block >> 32) & 0xFFFF_FFFF) as u32;
        cmd.cdw12 = (count as u32) - 1;

        let sq_ptr = (self.io_sq_phys as *mut NvmeCmd).wrapping_add(q.io_sq_tail);
        unsafe {
            core::ptr::write_volatile(sq_ptr, cmd);
        }

        q.io_sq_tail = (q.io_sq_tail + 1) % QUEUE_SIZE;
        self.ring_io_sq_doorbell(1, q.io_sq_tail as u32);

        // Wait on I/O CQ
        let cq_ptr = (self.io_cq_phys as *const NvmeCqe).wrapping_add(q.io_cq_head);
        let mut timeout = 100_000;
        loop {
            let cqe = unsafe { core::ptr::read_volatile(cq_ptr) };
            let phase = ((cqe.status & 1) != 0) as u8;
            if phase == q.io_cq_phase {
                q.io_cq_head = (q.io_cq_head + 1) % QUEUE_SIZE;
                if q.io_cq_head == 0 {
                    q.io_cq_phase ^= 1;
                }
                self.ring_io_cq_doorbell(1, q.io_cq_head as u32);

                if (cqe.status >> 1) != 0 {
                    return Err("NVMe Write Command failed");
                }
                break;
            }
            timeout -= 1;
            if timeout == 0 {
                return Err("NVMe Write Command timed out");
            }
            core::hint::spin_loop();
        }

        Ok(())
    }
}

impl BlockDevice for NvmeDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn block_size(&self) -> usize {
        self.sector_size
    }

    fn total_blocks(&self) -> u64 {
        self.total_sectors
    }

    fn read_blocks(&self, start_block: u64, count: usize, buf: &mut [u8]) -> Result<(), &'static str> {
        self.read_blocks_dma(start_block, count, buf)
    }

    fn write_blocks(&self, start_block: u64, count: usize, buf: &[u8]) -> Result<(), &'static str> {
        self.write_blocks_dma(start_block, count, buf)
    }
}

pub fn init() {
    lunix_println!("[storage] Scanning for NVMe PCIe Storage Controllers...");

    let pci_dev = match pci::find_by_class(0x01, 0x08)
        .or_else(|| pci::find_device(0x1B36, 0x0010))
        .or_else(|| pci::find_device(0x8086, 0x0953))
        .or_else(|| pci::find_device(0x8086, 0x0A54)) {
        Some(dev) => dev,
        None => {
            lunix_serial_println!("  [NVMe] No PCI NVMe controller detected.");
            return;
        }
    };

    pci::enable_bus_mastering(&pci_dev);

    let bar0_phys = if (pci_dev.bar0 & 6) == 4 {
        ((pci_dev.bar1 as u64) << 32) | ((pci_dev.bar0 & 0xFFFF_FFF0) as u64)
    } else {
        (pci_dev.bar0 & 0xFFFF_FFF0) as u64
    };

    let bar0_virt = match vmm::map_mmio_range(PhysAddr::new(bar0_phys), 16 * 1024) {
        Ok(v) => v.as_u64(),
        Err(_) => {
            lunix_serial_println!("  [NVMe] Failed to map BAR0 memory range");
            return;
        }
    };

    lunix_serial_println!("  [NVMe] Found Controller at {:X}:{:X}.0, BAR0=0x{:X}", pci_dev.bus, pci_dev.device, bar0_phys);

    let _cap_low = unsafe { core::ptr::read_volatile(bar0_virt as *const u32) };
    let cap_high = unsafe { core::ptr::read_volatile((bar0_virt + 4) as *const u32) };
    let db_stride = ((cap_high >> 0) & 0x0F) as usize;

    // 1. Reset Controller: CC.EN = 0
    unsafe {
        let mut cc = core::ptr::read_volatile((bar0_virt + 0x14) as *const u32);
        cc &= !1;
        core::ptr::write_volatile((bar0_virt + 0x14) as *mut u32, cc);

        let mut timeout = 2_000_000;
        while (core::ptr::read_volatile((bar0_virt + 0x1C) as *const u32) & 1) != 0 {
            timeout -= 1;
            if timeout == 0 {
                break;
            }
            core::hint::spin_loop();
        }
    }

    // 2. Allocate Admin Queue Frames
    let admin_sq_frame = match pmm::alloc_frame() {
        Some(f) => f.as_u64(),
        None => return,
    };
    let admin_cq_frame = match pmm::alloc_frame() {
        Some(f) => f.as_u64(),
        None => return,
    };
    let io_sq_frame = match pmm::alloc_frame() {
        Some(f) => f.as_u64(),
        None => return,
    };
    let io_cq_frame = match pmm::alloc_frame() {
        Some(f) => f.as_u64(),
        None => return,
    };

    unsafe {
        core::ptr::write_bytes(admin_sq_frame as *mut u8, 0, 4096);
        core::ptr::write_bytes(admin_cq_frame as *mut u8, 0, 4096);
        core::ptr::write_bytes(io_sq_frame as *mut u8, 0, 4096);
        core::ptr::write_bytes(io_cq_frame as *mut u8, 0, 4096);

        // AQA: ASQS = QUEUE_SIZE - 1 (bits 0-11), ACQS = QUEUE_SIZE - 1 (bits 16-27)
        let aqa = ((QUEUE_SIZE as u32 - 1) & 0xFFF) | (((QUEUE_SIZE as u32 - 1) & 0xFFF) << 16);
        core::ptr::write_volatile((bar0_virt + 0x24) as *mut u32, aqa);

        // ASQ & ACQ addresses
        core::ptr::write_volatile((bar0_virt + 0x28) as *mut u64, admin_sq_frame);
        core::ptr::write_volatile((bar0_virt + 0x30) as *mut u64, admin_cq_frame);

        // CC: EN = 1 (bit 0), CSS = 0 (NVM, bits 4-6), MPS = 0 (4KB, bits 7-10), IOSQES = 6 (64B, bits 16-19), IOCQES = 4 (16B, bits 20-23)
        let cc = 1 | (6 << 16) | (4 << 20);
        core::ptr::write_volatile((bar0_virt + 0x14) as *mut u32, cc);

        // Wait for CSTS.RDY = 1
        let mut timeout = 2_000_000;
        while (core::ptr::read_volatile((bar0_virt + 0x1C) as *const u32) & 1) == 0 {
            timeout -= 1;
            if timeout == 0 {
                lunix_serial_println!("  [NVMe] Timeout waiting for controller ready");
                return;
            }
            core::hint::spin_loop();
        }
    }

    let mut dev = NvmeDevice {
        name: String::from("nvme0n1"),
        bar0_base: bar0_virt,
        total_sectors: 131072,
        sector_size: 512,
        admin_sq_phys: admin_sq_frame,
        admin_cq_phys: admin_cq_frame,
        io_sq_phys: io_sq_frame,
        io_cq_phys: io_cq_frame,
        db_stride,
        queues: Mutex::new(NvmeQueues {
            admin_sq_tail: 0,
            admin_cq_head: 0,
            admin_cq_phase: 1,
            io_sq_tail: 0,
            io_cq_head: 0,
            io_cq_phase: 1,
        }),
    };

    // 3. Create I/O Completion Queue (Admin Opcode 0x05)
    let mut cmd_create_cq = NvmeCmd::default();
    cmd_create_cq.cdw0 = NVME_ADMIN_CREATE_IO_CQ as u32;
    cmd_create_cq.prp1 = io_cq_frame;
    cmd_create_cq.cdw10 = ((QUEUE_SIZE as u32 - 1) << 16) | 1; // QSIZE, QID=1
    cmd_create_cq.cdw11 = 1; // Physically contiguous
    let _ = dev.submit_admin_cmd(cmd_create_cq);

    // 4. Create I/O Submission Queue (Admin Opcode 0x01)
    let mut cmd_create_sq = NvmeCmd::default();
    cmd_create_sq.cdw0 = NVME_ADMIN_CREATE_IO_SQ as u32;
    cmd_create_sq.prp1 = io_sq_frame;
    cmd_create_sq.cdw10 = ((QUEUE_SIZE as u32 - 1) << 16) | 1; // QSIZE, QID=1
    cmd_create_sq.cdw11 = (1 << 16) | 1; // CQID=1, Physically contiguous
    let _ = dev.submit_admin_cmd(cmd_create_sq);

    // 5. Identify Namespace 1
    if let Some(ident_frame) = pmm::alloc_frame() {
        let ident_phys = ident_frame.as_u64();
        let mut cmd_ident = NvmeCmd::default();
        cmd_ident.cdw0 = NVME_ADMIN_IDENTIFY as u32;
        cmd_ident.nsid = 1;
        cmd_ident.prp1 = ident_phys;
        cmd_ident.cdw10 = 0; // CNS = 0 (Identify Namespace)
        if dev.submit_admin_cmd(cmd_ident).is_ok() {
            let nsze = unsafe { *(ident_phys as *const u64) };
            if nsze > 0 {
                dev.total_sectors = nsze;
            }
        }
    }

    let dev_arc = Arc::new(dev);
    register_block_device(dev_arc.clone());
    NVME_DEVICES.lock().push(dev_arc);
    NVME_INITIALIZED.store(true, Ordering::SeqCst);

    lunix_println!("[+] NVMe PCIe Storage Driver initialized (/dev/nvme0n1 mounted).");
    lunix_serial_println!("[+] NVMe PCIe Storage Driver initialized (/dev/nvme0n1 mounted).");
}
