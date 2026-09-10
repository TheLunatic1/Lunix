use core::cell::UnsafeCell;
use crate::arch::x86_64::pic::{InterruptIndex, PICS};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

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

static IDT: SyncUnsafeCell<InterruptDescriptorTable> =
    SyncUnsafeCell::new(InterruptDescriptorTable::new());

pub fn init() {
    unsafe {
        let idt = &mut *IDT.get();
        *idt = InterruptDescriptorTable::new();
        idt.divide_error.set_handler_fn(divide_error_handler);
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(crate::arch::x86_64::gdt::DOUBLE_FAULT_IST_INDEX);

        // Hardware IRQ handlers
        idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);

        idt.load();
    }
}

extern "x86-interrupt" fn divide_error_handler(stack_frame: InterruptStackFrame) {
    use crate::arch::x86_64::serial::{write_hex, write_str};
    write_str("\n[EXCEPTION] DIVIDE BY ZERO, RIP: ");
    write_hex(stack_frame.instruction_pointer.as_u64());
    write_str(", RSP: ");
    write_hex(stack_frame.stack_pointer.as_u64());
    write_str("\n");
    crate::arch::x86_64::hlt_loop()
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    use crate::arch::x86_64::serial::{write_hex, write_str};
    write_str("[EXCEPTION] BREAKPOINT, RIP: ");
    write_hex(stack_frame.instruction_pointer.as_u64());
    write_str("\n");
}

extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: InterruptStackFrame) {
    use crate::arch::x86_64::serial::{write_hex, write_str};
    write_str("\n[EXCEPTION] INVALID OPCODE, RIP: ");
    write_hex(stack_frame.instruction_pointer.as_u64());
    write_str(", RSP: ");
    write_hex(stack_frame.stack_pointer.as_u64());
    write_str("\n");
    crate::arch::x86_64::hlt_loop()
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    use crate::arch::x86_64::serial::{write_hex, write_str};
    write_str("\n[EXCEPTION] GENERAL PROTECTION FAULT, Error Code: ");
    write_hex(error_code);
    write_str(", RIP: ");
    write_hex(stack_frame.instruction_pointer.as_u64());
    write_str(", RSP: ");
    write_hex(stack_frame.stack_pointer.as_u64());
    write_str(", RFLAGS: ");
    write_hex(stack_frame.cpu_flags.bits());
    write_str("\n");
    crate::arch::x86_64::hlt_loop()
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use crate::arch::x86_64::serial::{write_hex, write_str};
    use x86_64::registers::control::Cr2;

    write_str("\n[EXCEPTION] PAGE FAULT, CR2 (Fault Addr): ");
    write_hex(Cr2::read().map(|a| a.as_u64()).unwrap_or(0));
    write_str(", Error Code: ");
    write_hex(error_code.bits());
    write_str(", RIP: ");
    write_hex(stack_frame.instruction_pointer.as_u64());
    write_str(", RSP: ");
    write_hex(stack_frame.stack_pointer.as_u64());
    write_str("\n");
    crate::arch::x86_64::hlt_loop()
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    use crate::arch::x86_64::serial::{write_hex, write_str};
    write_str("\n[FATAL EXCEPTION] DOUBLE FAULT, RIP: ");
    write_hex(stack_frame.instruction_pointer.as_u64());
    write_str(", RSP: ");
    write_hex(stack_frame.stack_pointer.as_u64());
    write_str("\n");
    crate::arch::x86_64::hlt_loop()
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    crate::drivers::timer::on_tick();
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}


extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    crate::drivers::keyboard::on_interrupt();
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
