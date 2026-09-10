use core::cell::UnsafeCell;
use x86_64::instructions::segmentation::{Segment, CS, DS, ES, FS, GS, SS};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
const DOUBLE_FAULT_STACK_SIZE: usize = 4096 * 4;

struct SyncUnsafeCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for SyncUnsafeCell<T> {}

impl<T> SyncUnsafeCell<T> {
    const fn new(val: T) -> Self {
        Self(UnsafeCell::new(val))
    }
    #[inline(always)]
    fn get(&self) -> *mut T {
        self.0.get()
    }
}

static DOUBLE_FAULT_STACK: SyncUnsafeCell<[u8; DOUBLE_FAULT_STACK_SIZE]> =
    SyncUnsafeCell::new([0; DOUBLE_FAULT_STACK_SIZE]);
static TSS: SyncUnsafeCell<TaskStateSegment> = SyncUnsafeCell::new(TaskStateSegment::new());
static GDT: SyncUnsafeCell<GlobalDescriptorTable> =
    SyncUnsafeCell::new(GlobalDescriptorTable::new());

pub fn init() {
    unsafe {
        let gdt = &mut *GDT.get();
        let tss = &mut *TSS.get();

        *gdt = GlobalDescriptorTable::new();
        *tss = TaskStateSegment::new();

        let stack_start = VirtAddr::from_ptr(DOUBLE_FAULT_STACK.get());
        let stack_end = stack_start + (DOUBLE_FAULT_STACK_SIZE as u64);
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = stack_end;

        let code_selector = gdt.append(Descriptor::kernel_code_segment());
        let data_selector = gdt.append(Descriptor::kernel_data_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(tss));

        gdt.load();
        CS::set_reg(code_selector);
        DS::set_reg(data_selector);
        ES::set_reg(data_selector);
        SS::set_reg(data_selector);
        FS::set_reg(SegmentSelector::NULL);
        GS::set_reg(SegmentSelector::NULL);
        load_tss(tss_selector);
    }
}
