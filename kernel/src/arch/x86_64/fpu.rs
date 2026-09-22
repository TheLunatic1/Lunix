use x86_64::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};

/// Reset the current thread's x87/SSE state to power-on defaults (start of `execve`).
pub fn reset_user_state() {
    let mxcsr: u32 = 0x1F80;
    unsafe {
        core::arch::asm!("fninit", options(nostack));
        core::arch::asm!("ldmxcsr [{}]", in(reg) &mxcsr, options(nostack, readonly));
    }
}

pub fn init() {
    unsafe {
        // 1. Configure CR0: Enable coprocessor monitoring, disable emulation, clear task switch
        let mut cr0 = Cr0::read();
        cr0.remove(Cr0Flags::EMULATE_COPROCESSOR);
        cr0.insert(Cr0Flags::MONITOR_COPROCESSOR);
        cr0.remove(Cr0Flags::TASK_SWITCHED);
        cr0.insert(Cr0Flags::NUMERIC_ERROR);
        // Kernel writes must fault on read-only pages too: copy-on-write depends on it.
        cr0.insert(Cr0Flags::WRITE_PROTECT);
        Cr0::write(cr0);

        // 2. Configure CR4: Enable OSFXSR and OSXMMEXCPT
        let mut cr4 = Cr4::read();
        cr4.insert(Cr4Flags::OSFXSR);
        cr4.insert(Cr4Flags::OSXMMEXCPT_ENABLE);
        Cr4::write(cr4);

        // 3. Initialize x87 FPU state
        core::arch::asm!("fninit");

        // 4. Set default MXCSR (mask all SIMD floating point exceptions)
        let mxcsr: u32 = 0x1F80;
        core::arch::asm!("ldmxcsr [{}]", in(reg) &mxcsr);
    }
}
