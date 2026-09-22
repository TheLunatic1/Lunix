//! Demand-paged user mappings (`mmap`).
//!
//! `mmap` only records a virtual memory area; no memory is allocated and no file data is
//! read until the program first touches a page. The page-fault handler then fills the
//! page: zeroes for anonymous areas, or one page read from the file for file-backed ones.
//! This is what lets a program map a 170 MB shared library and pay only for the pages it
//! actually uses, as on Linux.
//!
//! Areas are keyed by the address space (CR3), so a `vfork` child that shares its parent's
//! page tables also shares its areas; `fork` copies them.

use crate::fs::file::{FileHandle, SeekFrom};
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{PageTable, PageTableFlags};
use x86_64::{PhysAddr, VirtAddr};

pub type SharedFile = Arc<Mutex<Box<dyn FileHandle>>>;

const USER_TOP: u64 = 0x0000_8000_0000_0000;
const PAGE: u64 = 4096;

#[derive(Clone)]
struct Vma {
    start: u64,
    end: u64,
    /// Backing file and the file offset that corresponds to `start`.
    file: Option<(SharedFile, u64)>,
}

static TABLE: Mutex<BTreeMap<u64, Vec<Vma>>> = Mutex::new(BTreeMap::new());

/// Start of every process's heap (`brk`).
const HEAP_BASE: u64 = 0x0000_6000_0000_0000;

/// Program break of each address space.
static BRK: Mutex<BTreeMap<u64, u64>> = Mutex::new(BTreeMap::new());

/// `brk(2)`: query (`request == 0`) or grow the heap of the current address space. Heap
/// pages are demand-zeroed like any anonymous mapping. Shrinking is ignored.
pub fn brk(request: u64) -> u64 {
    let cr3 = current_cr3();
    let cur = *BRK.lock().entry(cr3).or_insert(HEAP_BASE);
    if request == 0 || request <= cur || request >= USER_TOP {
        return cur;
    }
    let old_end = (cur + PAGE - 1) & !(PAGE - 1);
    let new_end = (request + PAGE - 1) & !(PAGE - 1);
    if new_end > old_end {
        map(old_end, new_end - old_end, None);
    }
    BRK.lock().insert(cr3, request);
    request
}

fn current_cr3() -> u64 {
    Cr3::read().0.start_address().as_u64()
}

/// Forget every area of an address space (used when a fresh address space is created).
pub fn reset(cr3: u64) {
    TABLE.lock().remove(&cr3);
    BRK.lock().remove(&cr3);
}

/// `fork`: the child starts with the same areas (its page tables were copied already).
pub fn fork(parent_cr3: u64, child_cr3: u64) {
    if parent_cr3 == child_cr3 {
        return;
    }
    let mut t = TABLE.lock();
    let copy = t.get(&parent_cr3).cloned().unwrap_or_default();
    t.insert(child_cr3, copy);
    let parent_brk = BRK.lock().get(&parent_cr3).copied();
    if let Some(b) = parent_brk {
        BRK.lock().insert(child_cr3, b);
    }
}

/// Clear the present PTE for `virt` in the *current* address space. The frame is not freed:
/// it may be device memory (framebuffer) or shared with a forked copy.
fn unmap_current(virt: u64) {
    let pml4 = unsafe { &mut *(current_cr3() as *mut PageTable) };
    let va = VirtAddr::new(virt);
    if pml4[va.p4_index()].is_unused() {
        return;
    }
    let pdpt = unsafe { &mut *(pml4[va.p4_index()].addr().as_u64() as *mut PageTable) };
    if pdpt[va.p3_index()].is_unused() {
        return;
    }
    let pd = unsafe { &mut *(pdpt[va.p3_index()].addr().as_u64() as *mut PageTable) };
    if pd[va.p2_index()].is_unused() {
        return;
    }
    let pt = unsafe { &mut *(pd[va.p2_index()].addr().as_u64() as *mut PageTable) };
    if !pt[va.p1_index()].is_unused() {
        pt[va.p1_index()].set_unused();
        x86_64::instructions::tlb::flush(va);
    }
}

/// Remove `[start, end)` from the areas of `cr3`, splitting partially covered areas, and
/// drop any pages already populated there (of the *current* address space).
fn remove_range_locked(areas: &mut Vec<Vma>, start: u64, end: u64) {
    let mut out: Vec<Vma> = Vec::with_capacity(areas.len() + 1);
    for a in areas.drain(..) {
        if a.end <= start || a.start >= end {
            out.push(a);
            continue;
        }
        if a.start < start {
            out.push(Vma { start: a.start, end: start, file: a.file.clone() });
        }
        if a.end > end {
            let shift = end - a.start;
            out.push(Vma {
                start: end,
                end: a.end,
                file: a.file.as_ref().map(|(f, off)| (f.clone(), off + shift)),
            });
        }
    }
    *areas = out;
    let mut p = start & !(PAGE - 1);
    while p < end {
        unmap_current(p);
        p += PAGE;
    }
}

/// Register a mapping of `len` bytes at `start` (page aligned) in the current address
/// space, replacing whatever was mapped there (MAP_FIXED semantics).
pub fn map(start: u64, len: u64, file: Option<(SharedFile, u64)>) {
    let end = (start + len + PAGE - 1) & !(PAGE - 1);
    let cr3 = current_cr3();
    let mut t = TABLE.lock();
    let areas = t.entry(cr3).or_default();
    remove_range_locked(areas, start, end);
    areas.push(Vma { start, end, file });
}

/// Like [`map`], for a *different* address space that is not active (exec builds the new
/// image before switching to it).
pub fn map_in(cr3: u64, start: u64, len: u64, file: Option<(SharedFile, u64)>) {
    let end = (start + len + PAGE - 1) & !(PAGE - 1);
    TABLE.lock().entry(cr3).or_default().push(Vma { start, end, file });
}

/// `munmap`.
pub fn unmap(start: u64, len: u64) {
    let end = (start + len + PAGE - 1) & !(PAGE - 1);
    let cr3 = current_cr3();
    let mut t = TABLE.lock();
    if let Some(areas) = t.get_mut(&cr3) {
        remove_range_locked(areas, start & !(PAGE - 1), end);
    }
}

/// Page-fault handler hook: populate the page containing `addr` if it lies in a mapped
/// area. Returns false when the address is not covered (a genuine fault).
pub fn handle_fault(addr: u64) -> bool {
    if addr >= USER_TOP {
        return false;
    }
    let page = addr & !(PAGE - 1);
    let cr3 = current_cr3();

    // Find the area; hold the table lock only while looking.
    let (file, file_off) = {
        let t = TABLE.lock();
        let Some(areas) = t.get(&cr3) else { return false };
        let Some(a) = areas.iter().find(|a| page >= a.start && page < a.end) else { return false };
        match &a.file {
            Some((f, base)) => (Some(f.clone()), base + (page - a.start)),
            None => (None, 0),
        }
    };

    let Some(frame) = crate::mm::pmm::alloc_frame() else { return false };
    let dst = frame.as_u64() as *mut u8;
    unsafe {
        core::ptr::write_bytes(dst, 0, PAGE as usize);
    }
    if let Some(f) = file {
        let mut h = f.lock();
        if h.seek(SeekFrom::Start(file_off)).is_ok() {
            let slice = unsafe { core::slice::from_raw_parts_mut(dst, PAGE as usize) };
            let mut got = 0usize;
            while got < slice.len() {
                match h.read(&mut slice[got..]) {
                    Ok(0) | Err(_) => break, // EOF: the rest of the page stays zero
                    Ok(n) => got += n,
                }
            }
        }
    }
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
    crate::mm::vmm::map_page(VirtAddr::new(page), PhysAddr::new(frame.as_u64()), flags).is_ok()
}
