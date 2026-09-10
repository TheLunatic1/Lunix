use crate::mm::pmm;
use spin::Mutex;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

pub struct BootFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for BootFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let frame_addr = pmm::alloc_frame()?;
        Some(PhysFrame::containing_address(frame_addr))
    }
}

pub static PAGE_MAPPER: Mutex<Option<OffsetPageTable<'static>>> = Mutex::new(None);

pub fn init(phys_mem_offset: u64) {
    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = VirtAddr::new(phys.as_u64()); // In UEFI environment identity mapped

    let level_4_table = unsafe { &mut *(virt.as_mut_ptr::<PageTable>()) };
    let mapper = unsafe { OffsetPageTable::new(level_4_table, VirtAddr::new(phys_mem_offset)) };

    *PAGE_MAPPER.lock() = Some(mapper);
}

pub fn map_page(virt_addr: VirtAddr, phys_addr: PhysAddr, flags: PageTableFlags) -> Result<(), &'static str> {
    let mut mapper_lock = PAGE_MAPPER.lock();
    let mapper = mapper_lock.as_mut().ok_or("VMM not initialized")?;
    let page: Page<Size4KiB> = Page::containing_address(virt_addr);
    let frame: PhysFrame<Size4KiB> = PhysFrame::containing_address(phys_addr);

    let mut frame_allocator = BootFrameAllocator;
    unsafe {
        mapper
            .map_to(page, frame, flags, &mut frame_allocator)
            .map_err(|_| "Failed to map page")?
            .flush();
    }
    Ok(())
}

pub fn map_mmio_range(phys_start: PhysAddr, size: usize) -> Result<VirtAddr, &'static str> {
    // Identity map MMIO pages with CacheDisable and WriteThrough flags
    let page_count = (size + 4095) / 4096;
    let flags = PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | PageTableFlags::NO_CACHE
        | PageTableFlags::WRITE_THROUGH;

    for i in 0..page_count {
        let offset = (i * 4096) as u64;
        let phys = PhysAddr::new(phys_start.as_u64() + offset);
        let virt = VirtAddr::new(phys.as_u64());

        // Attempt mapping if not already mapped
        let _ = map_page(virt, phys, flags);
    }

    Ok(VirtAddr::new(phys_start.as_u64()))
}
