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
        unsafe {
            core::ptr::write_bytes(frame_addr.as_u64() as *mut u8, 0, 4096);
        }
        Some(PhysFrame::containing_address(frame_addr))
    }
}

pub static PAGE_MAPPER: Mutex<Option<OffsetPageTable<'static>>> = Mutex::new(None);

pub fn init(phys_mem_offset: u64) {
    let (uefi_l4_frame, flags) = Cr3::read();
    let uefi_virt = VirtAddr::new(uefi_l4_frame.start_address().as_u64());
    let uefi_l4 = unsafe { &*(uefi_virt.as_ptr::<PageTable>()) };

    // 1. Allocate clean PML4 and PDPT for kernel
    let kernel_l4_frame = pmm::alloc_frame().expect("Failed to allocate kernel PML4 frame");
    let kernel_l4 = unsafe { &mut *(kernel_l4_frame.as_u64() as *mut PageTable) };
    kernel_l4.zero();

    let pdpt_frame = pmm::alloc_frame().expect("Failed to allocate PDPT frame");
    let pdpt = unsafe { &mut *(pdpt_frame.as_u64() as *mut PageTable) };
    pdpt.zero();

    // 2. Allocate and initialize 4 Page Directories mapping 4 GiB
    for pd_idx in 0..4 {
        let pd_frame = pmm::alloc_frame().expect("Failed to allocate PageDirectory frame");
        let pd = unsafe { &mut *(pd_frame.as_u64() as *mut PageTable) };
        pd.zero();

        for entry_idx in 0..512 {
            let pd_flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE;

            if pd_idx == 0 && entry_idx < 16 {
                // First 32 MiB (0x0..0x2000000): Map via 4 KiB Page Tables so individual pages can be mapped/isolated
                let pt_frame = pmm::alloc_frame().expect("Failed to allocate PT frame");
                let pt = unsafe { &mut *(pt_frame.as_u64() as *mut PageTable) };
                pt.zero();

                let pt_base = (entry_idx as u64) * (2 * 1024 * 1024);
                for pt_idx in 0..512 {
                    let phys = pt_base + (pt_idx as u64) * 4096;
                    let page_flags = PageTableFlags::PRESENT
                        | PageTableFlags::WRITABLE
                        | PageTableFlags::USER_ACCESSIBLE;
                    pt[pt_idx].set_addr(PhysAddr::new(phys), page_flags);
                }

                pd[entry_idx].set_addr(pt_frame, pd_flags);
            } else {
                let phys = ((pd_idx as u64) * 512 + entry_idx as u64) * (2 * 1024 * 1024);
                let page_flags = pd_flags | PageTableFlags::HUGE_PAGE;
                pd[entry_idx].set_addr(PhysAddr::new(phys), page_flags);
            }
        }

        let pdpt_flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::USER_ACCESSIBLE;

        pdpt[pd_idx].set_addr(pd_frame, pdpt_flags);
    }

    // 3. Connect PDPT to PML4[0]
    let l4_flags = PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | PageTableFlags::USER_ACCESSIBLE;
    kernel_l4[0].set_addr(pdpt_frame, l4_flags);

    // 4. Preserve any higher-half UEFI runtime mappings
    for i in 1..512 {
        if !uefi_l4[i].is_unused() {
            kernel_l4[i].set_addr(
                uefi_l4[i].addr(),
                uefi_l4[i].flags() | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
            );
        }
    }

    // 5. Activate Kernel PML4 in CR3
    let new_phys_frame = PhysFrame::containing_address(kernel_l4_frame);
    unsafe {
        Cr3::write(new_phys_frame, flags);
    }

    let mapper = unsafe { OffsetPageTable::new(kernel_l4, VirtAddr::new(phys_mem_offset)) };
    *PAGE_MAPPER.lock() = Some(mapper);
}

pub fn map_page(virt_addr: VirtAddr, phys_addr: PhysAddr, flags: PageTableFlags) -> Result<(), &'static str> {
    let mut mapper_lock = PAGE_MAPPER.lock();
    let mapper = mapper_lock.as_mut().ok_or("VMM not initialized")?;
    let page: Page<Size4KiB> = Page::containing_address(virt_addr);
    let frame: PhysFrame<Size4KiB> = PhysFrame::containing_address(phys_addr);

    let mut frame_allocator = BootFrameAllocator;
    unsafe {
        if let Ok((_old_frame, flusher)) = mapper.unmap(page) {
            flusher.flush();
        }
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
