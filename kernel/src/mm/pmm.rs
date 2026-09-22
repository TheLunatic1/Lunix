use core::sync::atomic::{AtomicUsize, Ordering};
use lunix_common::{BootInfo, MemoryRegionType, PAGE_SIZE};
use spin::Mutex;
use x86_64::PhysAddr;


pub const MAX_FRAMES: usize = 1024 * 1024; // 4 GiB physical memory tracking
pub const BITMAP_SIZE: usize = MAX_FRAMES / 64; // 16,384 u64 elements (128 KiB bitmap)

pub struct PhysicalMemoryManager {
    pub bitmap: [u64; BITMAP_SIZE],
    pub total_frames: usize,
    pub used_frames: usize,
    pub highest_frame: usize,
}

impl PhysicalMemoryManager {
    pub const fn new() -> Self {
        Self {
            bitmap: [!0u64; BITMAP_SIZE],
            total_frames: 0,
            used_frames: 0,
            highest_frame: 0,
        }
    }

    #[inline]
    pub fn set_frame_free(&mut self, frame_idx: usize) {
        if frame_idx < MAX_FRAMES {
            let idx = frame_idx / 64;
            let bit = frame_idx % 64;
            if (self.bitmap[idx] & (1 << bit)) != 0 {
                self.bitmap[idx] &= !(1 << bit);
                self.used_frames = self.used_frames.saturating_sub(1);
            }
        }
    }

    #[inline]
    pub fn set_frame_used(&mut self, frame_idx: usize) {
        if frame_idx < MAX_FRAMES {
            let idx = frame_idx / 64;
            let bit = frame_idx % 64;
            if (self.bitmap[idx] & (1 << bit)) == 0 {
                self.bitmap[idx] |= 1 << bit;
                self.used_frames += 1;
            }
        }
    }

    pub fn alloc_frame(&mut self) -> Option<PhysAddr> {
        for idx in 0..BITMAP_SIZE {
            if self.bitmap[idx] != !0u64 {
                for bit in 0..64 {
                    if (self.bitmap[idx] & (1 << bit)) == 0 {
                        let frame_idx = idx * 64 + bit;
                        self.set_frame_used(frame_idx);
                        let phys_addr = (frame_idx as u64) * PAGE_SIZE;
                        return Some(PhysAddr::new(phys_addr));
                    }
                }
            }
        }
        None
    }

    pub fn free_frame(&mut self, addr: PhysAddr) {
        let frame_idx = (addr.as_u64() / PAGE_SIZE) as usize;
        self.set_frame_free(frame_idx);
    }

    /// Allocate `n` physically-contiguous frames (e.g. for a DMA ring that a device's PFN
    /// register addresses as one region). Unlike `alloc_frame()`, which only guarantees a
    /// single free frame, this scans for a run of `n` consecutive free indices.
    pub fn alloc_frames_contig(&mut self, n: usize) -> Option<PhysAddr> {
        if n == 0 {
            return None;
        }
        let mut run_start = 0usize;
        let mut run_len = 0usize;
        for idx in 0..BITMAP_SIZE {
            let word = self.bitmap[idx];
            if word == !0u64 {
                run_len = 0;
                continue;
            }
            for bit in 0..64 {
                let frame_idx = idx * 64 + bit;
                if (word & (1 << bit)) == 0 {
                    if run_len == 0 {
                        run_start = frame_idx;
                    }
                    run_len += 1;
                    if run_len == n {
                        for f in run_start..run_start + n {
                            self.set_frame_used(f);
                        }
                        return Some(PhysAddr::new((run_start as u64) * PAGE_SIZE));
                    }
                } else {
                    run_len = 0;
                }
            }
        }
        None
    }
}

pub static PMM: Mutex<PhysicalMemoryManager> = Mutex::new(PhysicalMemoryManager::new());

/// Usable RAM ranges (start, end) from the firmware memory map. Anything outside them
/// (framebuffer, PCI BARs, ...) is device memory and must never be copied or freed.
const MAX_RAM_REGIONS: usize = 128;
static RAM_REGIONS: Mutex<([(u64, u64); MAX_RAM_REGIONS], usize)> = Mutex::new(([(0, 0); MAX_RAM_REGIONS], 0));

/// True if `phys` lies in usable system RAM.
pub fn is_ram(phys: u64) -> bool {
    let g = RAM_REGIONS.lock();
    g.0[..g.1].iter().any(|&(s, e)| phys >= s && phys < e)
}
static TOTAL_MEMORY_BYTES: AtomicUsize = AtomicUsize::new(0);
static USABLE_MEMORY_BYTES: AtomicUsize = AtomicUsize::new(0);

pub fn init(boot_info: &BootInfo) {
    use crate::arch::x86_64::serial::{write_dec, write_str};

    let memory_map = &boot_info.memory_map;
    write_str("[pmm::init] Regions: ");
    write_dec(memory_map.entry_count);
    write_str("\n");

    let mut pmm = PMM.lock();
    write_str("[pmm::init] Acquired PMM lock\n");

    let mut total_usable: u64 = 0;
    let mut total_mem: u64 = 0;

    for region in memory_map.iter() {
        total_mem += region.byte_size();

        if region.region_type == MemoryRegionType::Usable {
            total_usable += region.byte_size();
            {
                let mut g = RAM_REGIONS.lock();
                let n = g.1;
                if n < MAX_RAM_REGIONS {
                    g.0[n] = (region.phys_start, region.phys_start + region.byte_size());
                    g.1 = n + 1;
                }
            }

            let start_frame = (region.phys_start / PAGE_SIZE) as usize;
            let end_frame = ((region.phys_start + region.byte_size()) / PAGE_SIZE) as usize;

            for frame in start_frame..end_frame {
                if frame < MAX_FRAMES {
                    pmm.set_frame_free(frame);
                    if frame > pmm.highest_frame {
                        pmm.highest_frame = frame;
                    }
                }
            }
        }
    }

    // Protect first 32 MB of physical memory (IVT, BDA, BIOS/UEFI firmware, and Identity-mapped low memory)
    for frame in 0..(0x2000000 / PAGE_SIZE as usize) {
        pmm.set_frame_used(frame);
    }


    // Protect Kernel binary physical memory
    let kernel_start_frame = (boot_info.kernel_phys_base / PAGE_SIZE) as usize;
    let kernel_end_frame = ((boot_info.kernel_phys_base + boot_info.kernel_size + PAGE_SIZE - 1) / PAGE_SIZE) as usize;
    for frame in kernel_start_frame..kernel_end_frame {
        if frame < MAX_FRAMES {
            pmm.set_frame_used(frame);
        }
    }

    // Protect BootInfo memory
    let boot_info_start = (boot_info as *const BootInfo as u64) / PAGE_SIZE;
    let boot_info_end = (boot_info as *const BootInfo as u64 + core::mem::size_of::<BootInfo>() as u64 + PAGE_SIZE - 1) / PAGE_SIZE;
    for frame in (boot_info_start as usize)..(boot_info_end as usize) {
        if frame < MAX_FRAMES {
            pmm.set_frame_used(frame);
        }
    }

    pmm.total_frames = (total_mem / PAGE_SIZE) as usize;
    TOTAL_MEMORY_BYTES.store(total_mem as usize, Ordering::Relaxed);
    USABLE_MEMORY_BYTES.store(total_usable as usize, Ordering::Relaxed);
    
    write_str("[pmm::init] Usable RAM: ");
    write_dec((total_usable / (1024 * 1024)) as usize);
    write_str(" MB / Total RAM: ");
    write_dec((total_mem / (1024 * 1024)) as usize);
    write_str(" MB\n");
}

pub fn alloc_frame() -> Option<PhysAddr> {
    PMM.lock().alloc_frame()
}

pub fn free_frame(addr: PhysAddr) {
    PMM.lock().free_frame(addr);
}

/// See `PhysicalMemoryManager::alloc_frames_contig`.
pub fn alloc_frames_contig(n: usize) -> Option<PhysAddr> {
    PMM.lock().alloc_frames_contig(n)
}

/// Free `n` consecutive frames starting at `addr` (the counterpart to `alloc_frames_contig`).
pub fn free_frames_contig(addr: PhysAddr, n: usize) {
    let mut pmm = PMM.lock();
    for i in 0..n {
        pmm.free_frame(PhysAddr::new(addr.as_u64() + (i as u64) * PAGE_SIZE));
    }
}

pub fn get_memory_stats() -> (usize, usize, usize) {
    let pmm = PMM.lock();
    let total_bytes = TOTAL_MEMORY_BYTES.load(Ordering::Relaxed);
    let usable_bytes = USABLE_MEMORY_BYTES.load(Ordering::Relaxed);
    let used_bytes = pmm.used_frames * (PAGE_SIZE as usize);
    (total_bytes, usable_bytes, used_bytes)
}
