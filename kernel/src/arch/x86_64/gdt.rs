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

#[derive(Clone, Copy, Debug)]
pub struct Selectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub tss: SegmentSelector,
}

static mut SELECTORS: Option<Selectors> = None;

pub fn get_selectors() -> Selectors {
    unsafe { SELECTORS.expect("GDT not initialized") }
}

pub fn set_kernel_stack(stack: u64) {
    unsafe {
        let tss = &mut *TSS.get();
        tss.privilege_stack_table[0] = VirtAddr::new(stack);
    }
}

pub fn init() {
    unsafe {
        let gdt = &mut *GDT.get();
        let tss = &mut *TSS.get();

        *gdt = GlobalDescriptorTable::new();
        *tss = TaskStateSegment::new();

        let stack_start = VirtAddr::from_ptr(DOUBLE_FAULT_STACK.get());
        let stack_end = stack_start + (DOUBLE_FAULT_STACK_SIZE as u64);
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = stack_end;

        // GDT layout for standard x86_64 syscall/sysret:
        // Slot 1 (0x08): Kernel Code 64-bit
        // Slot 2 (0x10): Kernel Data 64-bit
        // Slot 3 (0x18 | 3 = 0x1B): User Data 64-bit
        // Slot 4 (0x20 | 3 = 0x23): User Code 64-bit
        // Slot 5 & 6 (0x28): Task State Segment
        let kernel_code = gdt.append(Descriptor::kernel_code_segment());
        let kernel_data = gdt.append(Descriptor::kernel_data_segment());
        let user_data = gdt.append(Descriptor::user_data_segment());
        let user_code = gdt.append(Descriptor::user_code_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(tss));

        SELECTORS = Some(Selectors {
            kernel_code,
            kernel_data,
            user_data,
            user_code,
            tss: tss_selector,
        });

        gdt.load();
        CS::set_reg(kernel_code);
        DS::set_reg(kernel_data);
        ES::set_reg(kernel_data);
        SS::set_reg(kernel_data);
        FS::set_reg(SegmentSelector::NULL);
        GS::set_reg(SegmentSelector::NULL);
        load_tss(tss_selector);
    }
}

