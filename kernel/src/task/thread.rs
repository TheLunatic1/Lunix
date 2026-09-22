use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

pub const DEFAULT_STACK_SIZE: usize = 64 * 1024; // 64 KiB

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Ready,
    Running,
    Sleeping(u64), // Wakeup tick count
    Blocked,
    Dead,
}

/// `fxsave` image: x87 + MMX + SSE state of a thread (512 bytes, 16-byte aligned).
#[repr(C, align(16))]
#[derive(Clone)]
pub struct FpuState(pub [u8; 512]);

impl FpuState {
    /// State after `fninit` with default MXCSR: what a new program starts with.
    pub fn initial() -> Self {
        let mut s = [0u8; 512];
        s[0..2].copy_from_slice(&0x037Fu16.to_le_bytes()); // FCW: all exceptions masked, 64-bit precision
        s[24..28].copy_from_slice(&0x1F80u32.to_le_bytes()); // MXCSR
        FpuState(s)
    }
}

pub struct Thread {
    pub id: usize,
    pub name: String,
    pub process_id: usize,
    pub state: ThreadState,
    pub priority: u8,
    pub quantum_remaining: u32,
    pub stack: Vec<u8>,
    pub rsp: u64,
    pub kernel_rsp_top: u64,
    pub fs_base: u64,
    pub entry_fn: Option<fn()>,
    pub is_user: bool,
    /// Saved FPU/SSE registers while the thread is not running.
    pub fpu: FpuState,
    /// `CLONE_CHILD_CLEARTID` / `set_tid_address`: on thread exit the kernel writes 0 here
    /// and wakes the futex (this is how `pthread_join` finds out the thread ended).
    pub clear_child_tid: u64,
}

impl Thread {
    /// Creates a new kernel thread with its own stack and context.
    pub fn new_kernel(id: usize, name: &str, entry_fn: fn(), priority: u8) -> Self {
        let stack = vec![0u8; DEFAULT_STACK_SIZE];
        let stack_top = (stack.as_ptr() as u64 + DEFAULT_STACK_SIZE as u64) & !0xF;

        // Initialize stack frame for context_switch
        // Layout:
        // [stack_top - 8]  = return address (thread_trampoline)
        // [stack_top - 16] = rbp (0)
        // [stack_top - 24] = rbx (0)
        // [stack_top - 32] = r12 (0)
        // [stack_top - 40] = r13 (0)
        // [stack_top - 48] = r14 (0)
        // [stack_top - 56] = r15 (0)
        // [stack_top - 64] = RFLAGS (0x202 = Interrupts Enabled)
        let initial_rsp = stack_top - 64;

        unsafe {
            let p = stack_top as *mut u64;
            p.sub(1).write(thread_trampoline as *const () as usize as u64); // Return address
            p.sub(2).write(0);                                // rbp
            p.sub(3).write(0);                                // rbx
            p.sub(4).write(0);                                // r12
            p.sub(5).write(0);                                // r13
            p.sub(6).write(0);                                // r14
            p.sub(7).write(0);                                // r15
            p.sub(8).write(0x202);                            // RFLAGS (popped by popfq)
        }

        Self {
            id,
            name: String::from(name),
            process_id: 0, // Kernel process
            state: ThreadState::Ready,
            priority,
            quantum_remaining: 10, // 10 ticks = 10ms
            stack,
            rsp: initial_rsp,
            kernel_rsp_top: stack_top,
            fs_base: 0,
            entry_fn: Some(entry_fn),
            is_user: false,
            fpu: FpuState::initial(),
            clear_child_tid: 0,
        }
    }

    /// Creates an idle kernel thread for background hlt loops.
    pub fn new_idle(id: usize) -> Self {
        Self::new_kernel(id, "idle", idle_task, 0)
    }
}

extern "C" fn thread_trampoline() -> ! {
    x86_64::instructions::interrupts::enable();
    crate::task::scheduler::run_current_thread_entry();
    crate::task::scheduler::exit_current_thread();
}

fn idle_task() {
    loop {
        x86_64::instructions::hlt();
        crate::task::scheduler::yield_now();
    }
}
