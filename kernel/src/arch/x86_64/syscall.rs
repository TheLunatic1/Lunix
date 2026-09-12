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
pub static LAST_USER_RIP: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
pub static LAST_USER_RSP: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

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
        "mov [rip + {last_user_rsp}], rsp",
        "mov [rip + {last_user_rip}], rcx",
        "mov rsp, [rip + {kernel_rsp}]",


        // Save User context on kernel stack:
        "push qword ptr [rip + {user_rsp}]", // [rsp + 80] = User RSP
        "push r11",                          // [rsp + 72] = User RFLAGS
        "push rcx",                          // [rsp + 64] = User RIP
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // Pass syscall parameters:
        // rax = syscall_num -> rdi
        // rdi = arg1        -> rsi
        // rsi = arg2        -> rdx
        // rdx = arg3        -> rcx
        // r10 = arg4        -> r8
        // r8  = arg5        -> r9
        // r9  = arg6        -> stack
        "push r9",
        "mov r9, r8",
        "mov r8, r10",
        "mov rcx, rdx",
        "mov rdx, rsi",
        "mov rsi, rdi",
        "mov rdi, rax",

        "call {dispatcher}",

        // Pop 7th argument
        "add rsp, 8",

        // Restore registers
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "pop rcx",           // Restore User RIP for sysret
        "pop r11",           // Restore User RFLAGS for sysret
        "pop rsp",           // Restore User RSP directly from kernel stack

        // Return to user mode (64-bit sysret)
        "sysretq",

        user_rsp = sym USER_RSP_SCRATCH,
        kernel_rsp = sym KERNEL_RSP_SCRATCH,
        last_user_rsp = sym LAST_USER_RSP,
        last_user_rip = sym LAST_USER_RIP,
        dispatcher = sym crate::syscall::syscall_dispatcher,
    );
}

