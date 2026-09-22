//! Copy-on-write support for `fork`.
//!
//! `fork` shares every RAM page between parent and child, mapped read-only and tagged with
//! a software bit. The first write by either side faults; the handler gives the writer a
//! private copy (or just restores write access if it is the last user of the frame).

use alloc::collections::BTreeMap;
use spin::Mutex;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::page_table::PageTableEntry;
use x86_64::structures::paging::{PageTable, PageTableFlags};
use x86_64::VirtAddr;

/// Software-defined PTE bit marking a copy-on-write page.
pub const COW_BIT: PageTableFlags = PageTableFlags::BIT_9;

const USER_TOP: u64 = 0x0000_8000_0000_0000;

/// Users of each shared frame. Frames used by a single address space have no entry.
static SHARED: Mutex<BTreeMap<u64, u32>> = Mutex::new(BTreeMap::new());

/// Record one more mapping of `phys`.
pub fn share(phys: u64) {
    let mut m = SHARED.lock();
    *m.entry(phys).or_insert(1) += 1;
}

/// Drop one mapping of `phys`.
fn release(phys: u64) {
    let mut m = SHARED.lock();
    match m.get_mut(&phys) {
        Some(c) if *c > 2 => *c -= 1,
        Some(_) => {
            m.remove(&phys);
        }
        None => {}
    }
}

/// Drop one reference to a frame that is going away. Returns true if the caller was the
/// last user and must free the frame; false if another address space still maps it.
pub fn drop_ref(phys: u64) -> bool {
    let mut m = SHARED.lock();
    match m.get_mut(&phys) {
        Some(c) if *c > 2 => {
            *c -= 1;
            false
        }
        Some(_) => {
            // Two users, one leaving: the remaining one becomes the sole owner.
            m.remove(&phys);
            false
        }
        None => true,
    }
}

fn is_shared(phys: u64) -> bool {
    SHARED.lock().get(&phys).map_or(false, |&c| c > 1)
}

/// Find the leaf PTE for `virt` in the current address space.
fn pte_of(virt: VirtAddr) -> Option<&'static mut PageTableEntry> {
    let pml4 = unsafe { &mut *(Cr3::read().0.start_address().as_u64() as *mut PageTable) };
    let e4 = &pml4[virt.p4_index()];
    if e4.is_unused() {
        return None;
    }
    let pdpt = unsafe { &mut *(e4.addr().as_u64() as *mut PageTable) };
    let e3 = &pdpt[virt.p3_index()];
    if e3.is_unused() || e3.flags().contains(PageTableFlags::HUGE_PAGE) {
        return None;
    }
    let pd = unsafe { &mut *(e3.addr().as_u64() as *mut PageTable) };
    let e2 = &pd[virt.p2_index()];
    if e2.is_unused() || e2.flags().contains(PageTableFlags::HUGE_PAGE) {
        return None;
    }
    let pt = unsafe { &mut *(e2.addr().as_u64() as *mut PageTable) };
    let e1 = &mut pt[virt.p1_index()];
    if e1.is_unused() {
        None
    } else {
        // Page tables are identity mapped and live as long as the address space.
        Some(unsafe { &mut *(e1 as *mut PageTableEntry) })
    }
}

/// Handle a write fault on a present page. Returns true if it was a COW page and has been
/// made writable (the faulting instruction can be retried).
pub fn handle_write_fault(addr: u64) -> bool {
    if addr >= USER_TOP {
        return false;
    }
    let va = VirtAddr::new(addr & !0xFFF);
    let Some(pte) = pte_of(va) else { return false };
    let flags = pte.flags();
    if !flags.contains(COW_BIT) || !flags.contains(PageTableFlags::PRESENT) {
        return false;
    }
    let phys = pte.addr();
    let new_flags = (flags | PageTableFlags::WRITABLE) - COW_BIT;

    if is_shared(phys.as_u64()) {
        // Still shared: this writer gets its own copy.
        let Some(frame) = crate::mm::pmm::alloc_frame() else { return false };
        unsafe {
            core::ptr::copy_nonoverlapping(phys.as_u64() as *const u8, frame.as_u64() as *mut u8, 4096);
        }
        release(phys.as_u64());
        pte.set_addr(x86_64::PhysAddr::new(frame.as_u64()), new_flags);
    } else {
        // Last user of the frame: no copy needed.
        pte.set_flags(new_flags);
    }
    x86_64::instructions::tlb::flush(va);
    true
}
