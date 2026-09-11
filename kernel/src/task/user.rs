//! Ring 3 User Mode transitions

const USER_DATA_SELECTOR: u64 = 0x1B; // GDT index 3 (0x18) | RPL 3
const USER_CODE_SELECTOR: u64 = 0x23; // GDT index 4 (0x20) | RPL 3
const DEFAULT_USER_RFLAGS: u64 = 0x202; // Interrupts enabled (IF), bit 1 set

/// Jumps into Ring 3 user space at entry_rip with user_rsp.
#[unsafe(naked)]
pub unsafe extern "C" fn enter_user_mode(entry_rip: u64, user_rsp: u64) -> ! {
    core::arch::naked_asm!(
        // rdi = entry_rip, rsi = user_rsp
        // Set DS/ES/FS/GS to User Data selector
        "mov ax, 0x1B",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",

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
