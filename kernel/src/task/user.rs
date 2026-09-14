//! Ring 3 User Mode transitions

const USER_DATA_SELECTOR: u64 = 0x1B; // GDT index 3 (0x18) | RPL 3
const USER_CODE_SELECTOR: u64 = 0x23; // GDT index 4 (0x20) | RPL 3
const DEFAULT_USER_RFLAGS: u64 = 0x202; // Interrupts enabled (IF), bit 1 set

/// Jumps into Ring 3 user space at entry_rip with user_rsp.
#[unsafe(naked)]
pub unsafe extern "C" fn enter_user_mode(entry_rip: u64, user_rsp: u64) -> ! {
    core::arch::naked_asm!(
        // rdi = entry_rip, rsi = user_rsp
        // Set DS/ES to User Data selector
        "mov ax, 0x1B",
        "mov ds, ax",
        "mov es, ax",

        "xor rax, rax",        // Set RAX to 0

        // Push iretq stack frame:
        // [rsp + 32] = SS (User Data)
        // [rsp + 24] = RSP (User Stack)
        // [rsp + 16] = RFLAGS
        // [rsp + 8]  = CS (User Code)
        // [rsp + 0]  = RIP (Entry Point)
        "push 0x1B",           // User SS
        "push rsi",            // User RSP
        "push 0x202",          // RFLAGS (IF=1)
        "push 0x23",           // User CS
        "push rdi",            // User RIP

        // Transition from Ring 0 to Ring 3
        "iretq",
    );
}

/// Jumps into Ring 3 user space at entry_rip with user_rsp and sets RAX to rax_val.
#[unsafe(naked)]
pub unsafe extern "C" fn enter_user_mode_with_rax(entry_rip: u64, user_rsp: u64, rax_val: u64) -> ! {
    core::arch::naked_asm!(
        // rdi = entry_rip, rsi = user_rsp, rdx = rax_val
        // Set DS/ES to User Data selector
        "mov ax, 0x1B",
        "mov ds, ax",
        "mov es, ax",

        "mov rax, rdx",        // Set RAX

        // Push iretq stack frame:
        // [rsp + 32] = SS (User Data)
        // [rsp + 24] = RSP (User Stack)
        // [rsp + 16] = RFLAGS
        // [rsp + 8]  = CS (User Code)
        // [rsp + 0]  = RIP (Entry Point)
        "push 0x1B",           // User SS
        "push rsi",            // User RSP
        "push 0x202",          // RFLAGS (IF=1)
        "push 0x23",           // User CS
        "push rdi",            // User RIP

        // Transition from Ring 0 to Ring 3
        "iretq",
    );
}

/// Jumps into Ring 3 user space at entry_rip with user_rsp and sets RAX, RDI, RSI.
#[unsafe(naked)]
pub unsafe extern "C" fn enter_user_mode_with_regs(
    entry_rip: u64,
    user_rsp: u64,
    rax_val: u64,
    rdi_val: u64,
    rsi_val: u64,
) -> ! {
    core::arch::naked_asm!(
        // rdi = entry_rip, rsi = user_rsp, rdx = rax_val, rcx = rdi_val, r8 = rsi_val
        "mov ax, 0x1B",
        "mov ds, ax",
        "mov es, ax",

        "mov r9, rsi",          // Save user_rsp in r9
        "mov rax, rdx",         // Set RAX to rax_val
        "mov rsi, r8",          // Set RSI to rsi_val
        "mov r10, rdi",         // Save entry_rip in r10
        "mov rdi, rcx",         // Set RDI to rdi_val

        // Push iretq stack frame:
        "push 0x1B",            // User SS
        "push r9",              // User RSP
        "push 0x202",           // RFLAGS (IF=1)
        "push 0x23",            // User CS
        "push r10",             // User RIP

        // Transition from Ring 0 to Ring 3
        "iretq",
    );
}

/// Jumps into Ring 3 user space restoring all registers from UserContext.
#[unsafe(naked)]
pub unsafe extern "C" fn enter_user_mode_full(ctx_ptr: *const crate::arch::x86_64::syscall::UserContext) -> ! {
    core::arch::naked_asm!(
        // rdi = ctx_ptr
        "mov ax, 0x1B",
        "mov ds, ax",
        "mov es, ax",

        // Push iretq stack frame:
        // [rsp + 32] = SS (0x1B)
        // [rsp + 24] = RSP ([rdi + 112])
        // [rsp + 16] = RFLAGS ([rdi + 104])
        // [rsp + 8]  = CS (0x23)
        // [rsp + 0]  = RIP ([rdi + 96])
        "push 0x1B",
        "push qword ptr [rdi + 112]", // rsp
        "push qword ptr [rdi + 104]", // rflags
        "push 0x23",
        "push qword ptr [rdi + 96]",  // rip

        // Restore all GPRs
        "mov r15, [rdi + 0]",
        "mov r14, [rdi + 8]",
        "mov r13, [rdi + 16]",
        "mov r12, [rdi + 24]",
        "mov rbp, [rdi + 32]",
        "mov rbx, [rdi + 40]",
        "mov r10, [rdi + 48]",
        "mov r9,  [rdi + 56]",
        "mov r8,  [rdi + 64]",
        "mov rdx, [rdi + 72]",
        "mov rsi, [rdi + 80]",
        "mov rax, [rdi + 128]",       // rax (0 for child)
        "mov rdi, [rdi + 88]",       // rdi loaded last

        "iretq",
    );
}

