#![no_std]

pub const LUNIX_BOOT_MAGIC: u64 = 0x4C554E49585F4F53; // "LUNIX_OS"
pub const HIGHER_HALF_OFFSET: u64 = 0xFFFF_8000_0000_0000;
pub const PAGE_SIZE: u64 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum PixelFormat {
    Rgb,
    Bgr,
    Bitmask,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FramebufferInfo {
    pub base_address: u64,
    pub size: usize,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub format: PixelFormat,
    pub bytes_per_pixel: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum MemoryRegionType {
    Usable,
    Reserved,
    AcpiReclaimable,
    AcpiNvs,
    Mmio,
    MmioPortSpace,
    PalCode,
    PersistentMemory,
    BootloaderCode,
    BootloaderData,
    KernelCode,
    KernelData,
    PageTables,
    BadMemory,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MemoryRegion {
    pub phys_start: u64,
    pub page_count: u64,
    pub region_type: MemoryRegionType,
}

impl MemoryRegion {
    pub const fn empty() -> Self {
        Self {
            phys_start: 0,
            page_count: 0,
            region_type: MemoryRegionType::Reserved,
        }
    }

    pub fn byte_size(&self) -> u64 {
        self.page_count * PAGE_SIZE
    }

    pub fn phys_end(&self) -> u64 {
        self.phys_start + self.byte_size()
    }
}

pub const MAX_MEMORY_REGIONS: usize = 128;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct MemoryMap {
    pub entry_count: usize,
    pub entries: [MemoryRegion; MAX_MEMORY_REGIONS],
}

impl MemoryMap {
    pub const fn new() -> Self {
        Self {
            entry_count: 0,
            entries: [MemoryRegion::empty(); MAX_MEMORY_REGIONS],
        }
    }

    pub fn add_region(&mut self, region: MemoryRegion) -> bool {
        if self.entry_count < MAX_MEMORY_REGIONS {
            self.entries[self.entry_count] = region;
            self.entry_count += 1;
            true
        } else {
            false
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &MemoryRegion> {
        self.entries[..self.entry_count].iter()
    }

    pub fn total_usable_memory(&self) -> u64 {
        self.iter()
            .filter(|r| r.region_type == MemoryRegionType::Usable)
            .map(|r| r.byte_size())
            .sum()
    }

    pub fn total_memory(&self) -> u64 {
        self.iter().map(|r| r.byte_size()).sum()
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct BootInfo {
    pub magic: u64,
    pub framebuffer: FramebufferInfo,
    pub memory_map: MemoryMap,
    pub rsdp_addr: Option<u64>,
    pub kernel_phys_base: u64,
    pub kernel_virt_base: u64,
    pub kernel_size: u64,
    pub physical_memory_offset: u64,
}

impl BootInfo {
    pub fn is_valid(&self) -> bool {
        self.magic == LUNIX_BOOT_MAGIC
    }
}
