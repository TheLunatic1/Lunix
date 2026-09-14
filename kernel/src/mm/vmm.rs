use crate::mm::pmm;
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{PageTable, PageTableFlags, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

pub static KERNEL_PML4_PHYS: AtomicU64 = AtomicU64::new(0);

pub fn init(_phys_mem_offset: u64) {
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

    KERNEL_PML4_PHYS.store(kernel_l4_frame.as_u64(), Ordering::SeqCst);

    // 5. Activate Kernel PML4 in CR3
    let new_phys_frame = PhysFrame::containing_address(kernel_l4_frame);
    unsafe {
        Cr3::write(new_phys_frame, flags);
    }
}

pub fn get_kernel_pml4() -> PhysAddr {
    PhysAddr::new(KERNEL_PML4_PHYS.load(Ordering::Relaxed))
}

pub fn map_page_in_pml4(
    pml4_phys: PhysAddr,
    virt_addr: VirtAddr,
    phys_addr: PhysAddr,
    flags: PageTableFlags,
) -> Result<(), &'static str> {
    let pml4 = unsafe { &mut *(pml4_phys.as_u64() as *mut PageTable) };
    let p4_idx = virt_addr.p4_index();

    // 1. Level 4 -> Level 3 (PDPT)
    let pdpt_phys = if pml4[p4_idx].is_unused() {
        let frame = pmm::alloc_frame().ok_or("OOM allocating PDPT")?;
        unsafe {
            core::ptr::write_bytes(frame.as_u64() as *mut u8, 0, 4096);
        }
        pml4[p4_idx].set_addr(
            frame,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
        );
        frame
    } else {
        pml4[p4_idx].addr()
    };

    // 2. Level 3 -> Level 2 (PD)
    let pdpt = unsafe { &mut *(pdpt_phys.as_u64() as *mut PageTable) };
    let p3_idx = virt_addr.p3_index();
    let pd_phys = if pdpt[p3_idx].is_unused() {
        let frame = pmm::alloc_frame().ok_or("OOM allocating PD")?;
        unsafe {
            core::ptr::write_bytes(frame.as_u64() as *mut u8, 0, 4096);
        }
        pdpt[p3_idx].set_addr(
            frame,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
        );
        frame
    } else {
        pdpt[p3_idx].addr()
    };

    // 3. Level 2 -> Level 1 (PT)
    let pd = unsafe { &mut *(pd_phys.as_u64() as *mut PageTable) };
    let p2_idx = virt_addr.p2_index();
    let pt_phys = if pd[p2_idx].is_unused() {
        let frame = pmm::alloc_frame().ok_or("OOM allocating PT")?;
        unsafe {
            core::ptr::write_bytes(frame.as_u64() as *mut u8, 0, 4096);
        }
        pd[p2_idx].set_addr(
            frame,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
        );
        frame
    } else {
        pd[p2_idx].addr()
    };

    // 4. Level 1 -> Physical Page Frame
    let pt = unsafe { &mut *(pt_phys.as_u64() as *mut PageTable) };
    let p1_idx = virt_addr.p1_index();
    pt[p1_idx].set_addr(phys_addr, flags);

    Ok(())
}

pub fn get_page_phys_in_pml4(pml4_phys: PhysAddr, virt_addr: VirtAddr) -> Option<PhysAddr> {
    let pml4 = unsafe { &*(pml4_phys.as_u64() as *const PageTable) };
    let p4_idx = virt_addr.p4_index();
    if pml4[p4_idx].is_unused() { return None; }
    let pdpt = unsafe { &*(pml4[p4_idx].addr().as_u64() as *const PageTable) };
    let p3_idx = virt_addr.p3_index();
    if pdpt[p3_idx].is_unused() { return None; }
    let pd = unsafe { &*(pdpt[p3_idx].addr().as_u64() as *const PageTable) };
    let p2_idx = virt_addr.p2_index();
    if pd[p2_idx].is_unused() { return None; }
    let pt = unsafe { &*(pd[p2_idx].addr().as_u64() as *const PageTable) };
    let p1_idx = virt_addr.p1_index();
    if pt[p1_idx].is_unused() { return None; }
    Some(pt[p1_idx].addr())
}

pub fn create_process_pml4() -> Result<PhysFrame<Size4KiB>, &'static str> {
    let proc_pml4_frame = pmm::alloc_frame().ok_or("OOM for process PML4")?;
    unsafe {
        core::ptr::write_bytes(proc_pml4_frame.as_u64() as *mut u8, 0, 4096);
    }
    let proc_pml4 = unsafe { &mut *(proc_pml4_frame.as_u64() as *mut PageTable) };

    let kernel_pml4_phys = KERNEL_PML4_PHYS.load(Ordering::SeqCst);
    let kernel_pml4 = unsafe { &*(kernel_pml4_phys as *const PageTable) };

    // 1. Copy higher-half entries
    for i in 1..512 {
        if !kernel_pml4[i].is_unused() {
            proc_pml4[i].set_addr(kernel_pml4[i].addr(), kernel_pml4[i].flags());
        }
    }

    // 2. Allocate private PDPT for lower-half (0..512 GiB)
    let proc_pdpt_frame = pmm::alloc_frame().ok_or("OOM for process PDPT")?;
    unsafe {
        core::ptr::write_bytes(proc_pdpt_frame.as_u64() as *mut u8, 0, 4096);
    }
    let proc_pdpt = unsafe { &mut *(proc_pdpt_frame.as_u64() as *mut PageTable) };

    let kernel_pdpt_phys = kernel_pml4[0].addr();
    let kernel_pdpt = unsafe { &*(kernel_pdpt_phys.as_u64() as *const PageTable) };

    // 3. Share 1 GiB .. 4 GiB direct identity mapping from kernel PDPT
    for i in 1..4 {
        if !kernel_pdpt[i].is_unused() {
            proc_pdpt[i].set_addr(kernel_pdpt[i].addr(), kernel_pdpt[i].flags());
        }
    }

    // 4. Allocate private PD0 for 0..1 GiB
    let proc_pd0_frame = pmm::alloc_frame().ok_or("OOM for proc PD0")?;
    unsafe {
        core::ptr::write_bytes(proc_pd0_frame.as_u64() as *mut u8, 0, 4096);
    }
    let proc_pd0 = unsafe { &mut *(proc_pd0_frame.as_u64() as *mut PageTable) };

    let kernel_pd0_phys = kernel_pdpt[0].addr();
    let kernel_pd0 = unsafe { &*(kernel_pd0_phys.as_u64() as *const PageTable) };

    // Share kernel code/heap/RAM (32 MiB .. 1 GiB)
    for i in 16..512 {
        if !kernel_pd0[i].is_unused() {
            proc_pd0[i].set_addr(kernel_pd0[i].addr(), kernel_pd0[i].flags());
        }
    }

    // Allocate private PT0 for 0..2 MiB (low-memory BIOS/AP trampoline/IVT)
    let proc_pt0_frame = pmm::alloc_frame().ok_or("OOM for proc PT0")?;
    unsafe {
        core::ptr::write_bytes(proc_pt0_frame.as_u64() as *mut u8, 0, 4096);
    }
    let proc_pt0 = unsafe { &mut *(proc_pt0_frame.as_u64() as *mut PageTable) };
    let kernel_pt0_phys = kernel_pd0[0].addr();
    let kernel_pt0 = unsafe { &*(kernel_pt0_phys.as_u64() as *const PageTable) };

    for i in 0..512 {
        if !kernel_pt0[i].is_unused() {
            proc_pt0[i].set_addr(kernel_pt0[i].addr(), kernel_pt0[i].flags());
        }
    }

    proc_pd0[0].set_addr(
        proc_pt0_frame,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
    );
    proc_pdpt[0].set_addr(
        proc_pd0_frame,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
    );
    proc_pml4[0].set_addr(
        proc_pdpt_frame,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
    );

    Ok(PhysFrame::containing_address(proc_pml4_frame))
}

pub fn clone_process_pml4(parent_pml4_phys: PhysAddr) -> Result<PhysFrame<Size4KiB>, &'static str> {
    let child_pml4_frame = create_process_pml4()?;
    let child_pml4_phys = child_pml4_frame.start_address();

    let parent_pml4 = unsafe { &*(parent_pml4_phys.as_u64() as *const PageTable) };

    // Scan user PML4 indices (0..256)
    for p4_idx in 0..256 {
        if parent_pml4[p4_idx].is_unused() {
            continue;
        }

        let pdpt_phys = parent_pml4[p4_idx].addr();
        let pdpt = unsafe { &*(pdpt_phys.as_u64() as *const PageTable) };

        for p3_idx in 0..512 {
            if pdpt[p3_idx].is_unused() {
                continue;
            }

            // If PML4[0] and p3_idx in 1..4, that is kernel identity 1GB..4GB, skip user cloning
            if p4_idx == 0 && p3_idx >= 1 && p3_idx < 4 {
                continue;
            }

            let pd_phys = pdpt[p3_idx].addr();
            let pd = unsafe { &*(pd_phys.as_u64() as *const PageTable) };

            for p2_idx in 0..512 {
                if pd[p2_idx].is_unused() {
                    continue;
                }

                // If huge page (bit 7 set), skip kernel huge page
                if pd[p2_idx].flags().contains(PageTableFlags::HUGE_PAGE) {
                    continue;
                }

                // If PML4[0], PDPT[0], and p2_idx in 16..512, that is kernel RAM 32MB..1GB, skip
                if p4_idx == 0 && p3_idx == 0 && p2_idx >= 16 {
                    continue;
                }

                let pt_phys = pd[p2_idx].addr();
                let pt = unsafe { &*(pt_phys.as_u64() as *const PageTable) };

                for p1_idx in 0..512 {
                    if pt[p1_idx].is_unused() {
                        continue;
                    }

                    // If in low 2MB (p4=0, p3=0, p2=0), only clone if user accessible and >= 0x10000 (above BIOS/trampoline)
                    if p4_idx == 0 && p3_idx == 0 && p2_idx == 0 && p1_idx < 16 {
                        continue;
                    }

                    let flags = pt[p1_idx].flags();
                    if flags.contains(PageTableFlags::USER_ACCESSIBLE) && flags.contains(PageTableFlags::PRESENT) {
                        let parent_page_phys = pt[p1_idx].addr();
                        let new_frame = pmm::alloc_frame().ok_or("OOM cloning user page frame")?;

                        // Copy 4 KiB contents from parent page to child page
                        unsafe {
                            core::ptr::copy_nonoverlapping(
                                parent_page_phys.as_u64() as *const u8,
                                new_frame.as_u64() as *mut u8,
                                4096,
                            );
                        }

                        // Compute canonical virtual address
                        let mut vaddr_u64 = ((p4_idx as u64) << 39)
                            | ((p3_idx as u64) << 30)
                            | ((p2_idx as u64) << 21)
                            | ((p1_idx as u64) << 12);
                        if (vaddr_u64 & (1 << 47)) != 0 {
                            vaddr_u64 |= 0xFFFF_0000_0000_0000;
                        }
                        let virt_addr = VirtAddr::new(vaddr_u64);

                        map_page_in_pml4(child_pml4_phys, virt_addr, new_frame, flags)?;
                    }
                }
            }
        }
    }

    Ok(child_pml4_frame)
}

pub fn is_page_mapped(virt_addr: VirtAddr) -> bool {
    let (cr3_frame, _) = Cr3::read();
    let pml4 = unsafe { &*(cr3_frame.start_address().as_u64() as *const PageTable) };
    let p4_idx = virt_addr.p4_index();
    if pml4[p4_idx].is_unused() {
        return false;
    }
    let pdpt = unsafe { &*(pml4[p4_idx].addr().as_u64() as *const PageTable) };
    let p3_idx = virt_addr.p3_index();
    if pdpt[p3_idx].is_unused() {
        return false;
    }
    let pd = unsafe { &*(pdpt[p3_idx].addr().as_u64() as *const PageTable) };
    let p2_idx = virt_addr.p2_index();
    if pd[p2_idx].is_unused() {
        return false;
    }
    let pt = unsafe { &*(pd[p2_idx].addr().as_u64() as *const PageTable) };
    let p1_idx = virt_addr.p1_index();
    !pt[p1_idx].is_unused()
}

pub fn map_page(virt_addr: VirtAddr, phys_addr: PhysAddr, flags: PageTableFlags) -> Result<(), &'static str> {
    let (cr3_frame, _) = Cr3::read();
    map_page_in_pml4(cr3_frame.start_address(), virt_addr, phys_addr, flags)?;
    x86_64::instructions::tlb::flush(virt_addr);
    Ok(())
}

pub fn map_mmio_range(phys_start: PhysAddr, size: usize) -> Result<VirtAddr, &'static str> {
    let page_count = (size + 4095) / 4096;
    let flags = PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | PageTableFlags::NO_CACHE
        | PageTableFlags::WRITE_THROUGH;

    for i in 0..page_count {
        let offset = (i * 4096) as u64;
        let phys = PhysAddr::new(phys_start.as_u64() + offset);
        let virt = VirtAddr::new(phys.as_u64());

        let _ = map_page(virt, phys, flags);
    }

    Ok(VirtAddr::new(phys_start.as_u64()))
}
