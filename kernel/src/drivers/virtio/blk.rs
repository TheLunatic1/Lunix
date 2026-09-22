//! VirtIO-Block Paravirtualized Storage Driver
//!
//! Controls high-speed VirtIO block devices in virtualized environments (QEMU/KVM),
//! providing Split VirtQueue ring buffers and fast DMA block transfers.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;
use crate::arch::x86_64::io::{inb, inl, inw, outb, outl, outw};
use crate::drivers::pci;
use crate::drivers::virtio::*;
use crate::fs::block::{register_block_device, BlockDevice};
use crate::mm::pmm;
use crate::lunix_println;

pub const VIRTIO_BLK_T_IN: u32 = 0;   // Read
pub const VIRTIO_BLK_T_OUT: u32 = 1;  // Write

pub const VIRTIO_BLK_S_OK: u8 = 0;
pub const VIRTIO_BLK_S_IOERR: u8 = 1;

/// Largest transfer done as a single virtio request/descriptor (256 sectors = 128 KiB, matching
/// the ATA driver's per-command run size). `read_blocks`/`write_blocks` split anything larger
/// into chunks of this size before calling the internal single-request functions.
const MAX_CHUNK_SECTORS: usize = 256;

// Note: the ring size used throughout this driver is always the device-reported `QueueNum`
// (read from the legacy virtio-pci `QUEUE_NUM` register at init), never a value the driver
// invents. The legacy transport gives the driver no way to ask for a smaller queue than the
// device already committed to when it laid out `QueueNum`-sized avail/used rings internally;
// using any other size desyncs the avail/used ring offsets from what the device computes,
// corrupting every transfer (this was a real bug here: a hardcoded 64-entry assumption against
// a device that actually reports a different QueueNum silently produced garbage reads).

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
    /// The device's actual `QueueNum` (ring entry count) — see the note on `QUEUE_SIZE_FALLBACK`.
    queue_size: usize,
    state: Mutex<VirtioBlkState>,
}

impl VirtioBlkDevice {
    /// Byte offset of the used ring within the queue region, matching exactly how the device
    /// itself lays out the same queue (legacy virtio queue layout, spec 2.4.2).
    fn used_ring_offset(queue_size: usize) -> u64 {
        let desc_bytes = (queue_size * core::mem::size_of::<VirtqDesc>()) as u64;
        let avail_bytes = 4 + 2 * queue_size as u64;
        (desc_bytes + avail_bytes + 4095) & !4095
    }
}

pub static VIRTIO_BLK_INITIALIZED: AtomicBool = AtomicBool::new(false);
pub static VIRTIO_BLK_DEVICES: Mutex<Vec<Arc<VirtioBlkDevice>>> = Mutex::new(Vec::new());

/// Frees a run of `n` DMA frames on drop, whichever way the function returns (including the
/// early `?`/`return Err` paths below). Without this, every single read/write call permanently
/// leaked its request and data frames — harmless for the handful of reads at boot, but under a
/// sustained heavy read workload (X server font/library loading, many concurrent file opens) it
/// exhausts physical memory over time with no error until something else's allocation fails.
struct DmaFrames {
    addr: u64,
    frames: usize,
}
impl DmaFrames {
    fn alloc(frames: usize) -> Result<Self, &'static str> {
        let addr = pmm::alloc_frames_contig(frames).ok_or("Failed to allocate VirtIO DMA frame(s)")?;
        Ok(Self { addr: addr.as_u64(), frames })
    }
}
impl Drop for DmaFrames {
    fn drop(&mut self) {
        pmm::free_frames_contig(x86_64::PhysAddr::new(self.addr), self.frames);
    }
}

impl VirtioBlkDevice {
    fn read_sectors_internal(&self, start_sector: u64, count: usize, buf: &mut [u8]) -> Result<(), &'static str> {
        let byte_len = count * 512;
        if buf.len() < byte_len {
            return Err("Destination buffer too small");
        }

        // One frame for the request header + status byte (both always fit in one page); the
        // data buffer is sized to the actual transfer, not a single fixed 4 KiB frame — a
        // request over 8 sectors (4 KiB) used to tell the device to DMA past the end of that
        // one frame into whatever physical memory happened to follow it. Both are freed when
        // dropped at the end of this function, on every return path.
        let req_dma = DmaFrames::alloc(1)?;
        let req_phys = req_dma.addr;
        let data_dma = DmaFrames::alloc((byte_len + 4095) / 4096)?;
        let data_phys = data_dma.addr;

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
            let avail_offset = (self.queue_size * core::mem::size_of::<VirtqDesc>()) as u64;
            let avail_ring_ptr = (self.queue_phys + avail_offset) as *mut u16;

            let cur_idx = state.avail_idx;
            *avail_ring_ptr.add(2 + (cur_idx as usize % self.queue_size)) = 0;
            state.avail_idx = state.avail_idx.wrapping_add(1);
            core::ptr::write_volatile(avail_ring_ptr.add(1), state.avail_idx);

            // Notify device
            outw(self.io_base + VIRTIO_PCI_QUEUE_NOTIFY, 0);

            // Poll Used Ring
            let used_offset = Self::used_ring_offset(self.queue_size);
            let used_ring_ptr = (self.queue_phys + used_offset) as *const u16;

            let mut timeout = 100_000_000u64;
            while core::ptr::read_volatile(used_ring_ptr.add(1)) == state.last_used_idx {
                timeout -= 1;
                if timeout == 0 {
                    return Err("VirtIO-Block read timeout");
                }
                core::hint::spin_loop();
            }

            state.last_used_idx = core::ptr::read_volatile(used_ring_ptr.add(1));

            // Legacy virtio-pci raises its INTx# line on every used-ring update regardless of
            // whether the driver actually waits on interrupts; reading the ISR status register
            // is what acknowledges and deasserts it (a side effect of the read, per the virtio
            // 1.0 spec's legacy-transport section). This driver only ever polls and never
            // installed an IRQ handler for this device, so skipping this read left the line
            // permanently asserted after the very first request — harmless-looking (this driver
            // doesn't need the interrupt), but if the INTx# line is electrically shared with
            // another device behind the same IOAPIC pin (common on QEMU's PCI INTx routing),
            // a stuck-asserted shared level-triggered line can suppress that other device's
            // interrupts too. Observed effect: fine at boot, but X/JWM/xterm startup (heavier
            // concurrent keyboard/timer/AF_UNIX activity) stalled indefinitely after this device
            // was added, with no fault or error logged anywhere.
            let _ = inb(self.io_base + VIRTIO_PCI_ISR);

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

        let req_dma = DmaFrames::alloc(1)?;
        let req_phys = req_dma.addr;
        let data_dma = DmaFrames::alloc((byte_len + 4095) / 4096)?;
        let data_phys = data_dma.addr;

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

            let avail_offset = (self.queue_size * core::mem::size_of::<VirtqDesc>()) as u64;
            let avail_ring_ptr = (self.queue_phys + avail_offset) as *mut u16;

            let cur_idx = state.avail_idx;
            *avail_ring_ptr.add(2 + (cur_idx as usize % self.queue_size)) = 0;
            state.avail_idx = state.avail_idx.wrapping_add(1);
            core::ptr::write_volatile(avail_ring_ptr.add(1), state.avail_idx);

            outw(self.io_base + VIRTIO_PCI_QUEUE_NOTIFY, 0);

            let used_offset = Self::used_ring_offset(self.queue_size);
            let used_ring_ptr = (self.queue_phys + used_offset) as *const u16;

            let mut timeout = 100_000_000u64;
            while core::ptr::read_volatile(used_ring_ptr.add(1)) == state.last_used_idx {
                timeout -= 1;
                if timeout == 0 {
                    return Err("VirtIO-Block write timeout");
                }
                core::hint::spin_loop();
            }

            state.last_used_idx = core::ptr::read_volatile(used_ring_ptr.add(1));

            // See the matching comment in read_sectors_internal: this read acknowledges and
            // deasserts the device's INTx# line.
            let _ = inb(self.io_base + VIRTIO_PCI_ISR);

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
        // One virtio request per chunk of up to MAX_CHUNK_SECTORS: bounds each request's DMA
        // buffer to a small, easy-to-satisfy contiguous allocation instead of needing one
        // arbitrarily large (and, as the caller's read grows, increasingly hard to find)
        // contiguous physical run for the whole transfer in a single descriptor.
        let mut done = 0usize;
        while done < count {
            let n = (count - done).min(MAX_CHUNK_SECTORS);
            self.read_sectors_internal(start_block + done as u64, n, &mut buf[done * 512..(done + n) * 512])?;
            done += n;
        }
        Ok(())
    }

    fn write_blocks(&self, start_block: u64, count: usize, buf: &[u8]) -> Result<(), &'static str> {
        let mut done = 0usize;
        while done < count {
            let n = (count - done).min(MAX_CHUNK_SECTORS);
            self.write_sectors_internal(start_block + done as u64, n, &buf[done * 512..(done + n) * 512])?;
            done += n;
        }
        Ok(())
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
        let q_num = inw(io_base + VIRTIO_PCI_QUEUE_NUM) as usize;
        if q_num == 0 {
            lunix_serial_println!("  [VirtIO-Blk] Queue 0 not available");
            return;
        }
        // The legacy transport gives the driver no say in queue size: QueueNum is the device's
        // own ring size, and desc/avail/used offsets must be computed from exactly that value
        // (see the note on QUEUE_SIZE_FALLBACK) or every transfer silently reads/writes the
        // wrong bytes.
        let used_end = VirtioBlkDevice::used_ring_offset(q_num) + 4 + 8 * q_num as u64;
        let frames_needed = ((used_end + 4095) / 4096) as usize;

        let q_frame = match pmm::alloc_frames_contig(frames_needed) {
            Some(f) => f.as_u64(),
            None => {
                lunix_serial_println!("  [VirtIO-Blk] Failed to allocate {} contiguous frame(s) for queue", frames_needed);
                return;
            }
        };

        core::ptr::write_bytes(q_frame as *mut u8, 0, frames_needed * 4096);

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
            queue_size: q_num,
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
