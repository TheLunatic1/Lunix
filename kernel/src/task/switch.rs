//! Low-level x86_64 thread context switching

#[no_mangle]
#[unsafe(naked)]
pub unsafe extern "C" fn context_switch(_old_rsp: *mut u64, _new_rsp: u64) {
    core::arch::naked_asm!(
        // 1. Save callee-saved registers of old thread
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "pushfq",

        // 2. Save old RSP into *old_rsp (rdi)
        "mov [rdi], rsp",

        // 3. Switch to new RSP (rsi)
        "mov rsp, rsi",

        // 4. Restore callee-saved registers of new thread
        "popfq",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",

        // 5. Jump into new thread (pops RIP from stack)
        "ret"
    );
}
