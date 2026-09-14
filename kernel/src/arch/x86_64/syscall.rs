use crate::arch::x86_64::io::{rdmsr, wrmsr};

const IA32_EFER: u32 = 0xC0000080;
const IA32_STAR: u32 = 0xC0000081;
const IA32_LSTAR: u32 = 0xC0000082;
const IA32_FMASK: u32 = 0xC0000084;

const EFER_SCE: u64 = 1 << 0; // System Call Extensions enable bit

#[repr(align(4096))]
struct SyscallStack([u8; 16384]);
static mut SYSCALL_STACK: SyscallStack = SyscallStack([0; 16384]);

static mut USER_RSP_SCRATCH: u64 = 0;
static mut KERNEL_RSP_SCRATCH: u64 = 0;
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct UserContext {
    pub r15: u64,     // 0
    pub r14: u64,     // 8
    pub r13: u64,     // 16
    pub r12: u64,     // 24
    pub rbp: u64,     // 32
    pub rbx: u64,     // 40
    pub r10: u64,     // 48
    pub r9: u64,      // 56
    pub r8: u64,      // 64
    pub rdx: u64,     // 72
    pub rsi: u64,     // 80
    pub rdi: u64,     // 88
    pub rip: u64,     // 96
    pub rflags: u64,  // 104
    pub rsp: u64,     // 112
    pub fs_base: u64, // 120
    pub rax: u64,     // 128
}

pub static CURRENT_USER_CONTEXT: spin::Mutex<UserContext> = spin::Mutex::new(UserContext {
    r15: 0, r14: 0, r13: 0, r12: 0, rbp: 0, rbx: 0,
    r10: 0, r9: 0, r8: 0, rdx: 0, rsi: 0, rdi: 0,
    rip: 0, rflags: 0, rsp: 0, fs_base: 0, rax: 0,
});

#[no_mangle]
pub extern "C" fn save_user_context(ctx_ptr: *const UserContext) {
    if !ctx_ptr.is_null() {
        let mut ctx = unsafe { *ctx_ptr };
        let fs_base = unsafe { crate::arch::x86_64::io::rdmsr(0xC000_0100) };
        ctx.fs_base = fs_base;
        *CURRENT_USER_CONTEXT.lock() = ctx;
    }
}

pub fn init() {
    unsafe {
        let stack_top = core::ptr::addr_of!(SYSCALL_STACK) as u64 + 16384 - 16;
        KERNEL_RSP_SCRATCH = stack_top;

        // 1. Enable System Call Extensions (SCE) in EFER
        let efer = rdmsr(IA32_EFER);
        wrmsr(IA32_EFER, efer | EFER_SCE);

        // 2. Configure STAR MSR
        // Bits 47:32 = Kernel CS (0x08) -> Kernel SS will be 0x10
        // Bits 63:48 = User Base (0x10) -> User SS will be (0x10+8)|3 = 0x1B, User CS will be (0x10+16)|3 = 0x23
        let star = (0x0010u64 << 48) | (0x0008u64 << 32);
        wrmsr(IA32_STAR, star);

        // 3. Configure LSTAR MSR to syscall_entry stub
        wrmsr(IA32_LSTAR, syscall_entry as *const () as usize as u64);

        // 4. Configure FMASK MSR to mask Interrupt Flag (IF, bit 9 = 0x200) on syscall entry
        wrmsr(IA32_FMASK, 0x200);
    }
}

pub fn set_kernel_syscall_stack(rsp: u64) {
    unsafe {
        KERNEL_RSP_SCRATCH = rsp;
    }
}

#[no_mangle]
#[unsafe(naked)]
pub unsafe extern "C" fn syscall_entry() {
    core::arch::naked_asm!(
        // Save user RSP into scratch and switch to kernel stack
        "mov [rip + {user_rsp}], rsp",
        "mov rsp, [rip + {kernel_rsp}]",

        // Save complete User register state on kernel stack (15 qwords = 120 bytes):
        "push qword ptr [rip + {user_rsp}]", // [rsp + 112] User RSP
        "push r11",                          // [rsp + 104] User RFLAGS
        "push rcx",                          // [rsp + 96]  User RIP
        "push rdi",                          // [rsp + 88]  User RDI
        "push rsi",                          // [rsp + 80]  User RSI
        "push rdx",                          // [rsp + 72]  User RDX
        "push r8",                           // [rsp + 64]  User R8
        "push r9",                           // [rsp + 56]  User R9
        "push r10",                          // [rsp + 48]  User R10
        "push rbx",                          // [rsp + 40]  User RBX
        "push rbp",                          // [rsp + 32]  User RBP
        "push r12",                          // [rsp + 24]  User R12
        "push r13",                          // [rsp + 16]  User R13
        "push r14",                          // [rsp + 8]   User R14
        "push r15",                          // [rsp + 0]   User R15

        // Save user context for clone/fork:
        "push rax",                          // preserve syscall number
        "mov rdi, rsp",
        "add rdi, 8",                        // point rdi to UserContext ([rsp + 8])
        "call {save_ctx}",
        "pop rax",                           // restore syscall number

        // Load arguments for syscall_dispatcher(num, a1, a2, a3, a4, a5, a6):
        // rax = num -> rdi
        // [rsp + 88] = a1 (rdi) -> rsi
        // [rsp + 80] = a2 (rsi) -> rdx
        // [rsp + 72] = a3 (rdx) -> rcx
        // [rsp + 48] = a4 (r10) -> r8
        // [rsp + 64] = a5 (r8)  -> r9
        // [rsp + 56] = a6 (r9)  -> stack (7th argument in SysV AMD64)
        "mov r11, [rsp + 56]",               // r9 (arg6)
        "mov r9,  [rsp + 64]",               // r8 (arg5)
        "mov r8,  [rsp + 48]",               // r10 (arg4)
        "mov rcx, [rsp + 72]",               // rdx (arg3)
        "mov rdx, [rsp + 80]",               // rsi (arg2)
        "mov rsi, [rsp + 88]",               // rdi (arg1)
        "mov rdi, rax",                      // num
        "push r11",                          // push 7th arg (arg6) on stack

        "call {dispatcher}",

        // Pop 7th argument
        "add rsp, 8",

        // Restore all user registers in reverse order:
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop rcx",                           // User RIP for sysret
        "pop r11",                           // User RFLAGS for sysret
        "pop rsp",                           // User RSP directly from kernel stack

        // Return to user mode (64-bit sysret)
        "sysretq",

        user_rsp = sym USER_RSP_SCRATCH,
        kernel_rsp = sym KERNEL_RSP_SCRATCH,
        save_ctx = sym save_user_context,
        dispatcher = sym crate::syscall::syscall_dispatcher,
    );
}

