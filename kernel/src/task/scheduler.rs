use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use spin::Mutex;

use crate::task::switch::context_switch;
use crate::task::thread::{Thread, ThreadState};

pub struct Scheduler {
    threads: Vec<Thread>,
    ready_queue: VecDeque<usize>,
    current_tid: usize,
    idle_tid: usize,
    next_tid: usize,
}

static SCHEDULER: Mutex<Option<Scheduler>> = Mutex::new(None);
static SCHEDULER_INITIALIZED: AtomicBool = AtomicBool::new(false);
static CURRENT_TID: AtomicUsize = AtomicUsize::new(0);

impl Scheduler {
    pub fn new() -> Self {
        Self {
            threads: Vec::new(),
            ready_queue: VecDeque::new(),
            current_tid: 0,
            idle_tid: 1,
            next_tid: 2,
        }
    }
}

pub fn init() {
    let mut sched = Scheduler::new();

    // 1. Create main kernel thread (TID 0)
    let main_thread = Thread {
        id: 0,
        name: String::from("kernel_main"),
        process_id: 0,
        state: ThreadState::Running,
        priority: 10,
        quantum_remaining: 10,
        stack: Vec::new(), // Uses bootloader/initial kernel stack
        rsp: 0,
        kernel_rsp_top: 0,
        entry_fn: None,
        is_user: false,
    };
    sched.threads.push(main_thread);

    // 2. Create idle thread (TID 1)
    let idle_thread = Thread::new_idle(1);
    sched.threads.push(idle_thread);

    *SCHEDULER.lock() = Some(sched);
    CURRENT_TID.store(0, Ordering::SeqCst);
    SCHEDULER_INITIALIZED.store(true, Ordering::SeqCst);
}

pub fn is_initialized() -> bool {
    SCHEDULER_INITIALIZED.load(Ordering::Relaxed)
}

pub fn current_tid() -> usize {
    CURRENT_TID.load(Ordering::Relaxed)
}

/// Spawns a new kernel thread and enqueues it to the ready queue.
pub fn spawn(name: &str, entry: fn(), priority: u8) -> usize {
    let mut lock = SCHEDULER.lock();
    let sched = lock.as_mut().expect("Scheduler not initialized");

    let tid = sched.next_tid;
    sched.next_tid += 1;

    let thread = Thread::new_kernel(tid, name, entry, priority);
    sched.threads.push(thread);
    sched.ready_queue.push_back(tid);

    tid
}

/// Invoked when a thread entry starts inside thread_trampoline
pub fn run_current_thread_entry() {
    let entry_fn = {
        let lock = SCHEDULER.lock();
        let sched = lock.as_ref().expect("Scheduler not initialized");
        let tid = CURRENT_TID.load(Ordering::SeqCst);
        sched
            .threads
            .iter()
            .find(|t| t.id == tid)
            .and_then(|t| t.entry_fn)
    };

    if let Some(entry) = entry_fn {
        entry();
    }
}

/// Terminates the calling thread.
pub fn exit_current_thread() -> ! {
    let tid = current_tid();
    {
        let mut lock = SCHEDULER.lock();
        let sched = lock.as_mut().expect("Scheduler not initialized");
        if let Some(thread) = sched.threads.iter_mut().find(|t| t.id == tid) {
            thread.state = ThreadState::Dead;
        }
    }

    schedule();

    // In case schedule returns for dead thread, loop in hlt
    loop {
        x86_64::instructions::hlt();
    }
}

/// Blocks the calling thread until unblock_thread is called.
pub fn block_current_thread() {
    let tid = current_tid();
    {
        let mut lock = SCHEDULER.lock();
        let sched = lock.as_mut().expect("Scheduler not initialized");
        if let Some(thread) = sched.threads.iter_mut().find(|t| t.id == tid) {
            thread.state = ThreadState::Blocked;
        }
    }
    schedule();
}

/// Unblocks a previously blocked thread, making it ready.
pub fn unblock_thread(tid: usize) {
    let mut lock = SCHEDULER.lock();
    let sched = lock.as_mut().expect("Scheduler not initialized");
    if let Some(thread) = sched.threads.iter_mut().find(|t| t.id == tid) {
        if thread.state == ThreadState::Blocked {
            thread.state = ThreadState::Ready;
            sched.ready_queue.push_back(tid);
        }
    }
}

/// Puts current thread to sleep for `ms` milliseconds.
pub fn sleep_ms(ms: u64) {
    let current_ticks = crate::drivers::timer::get_ticks();
    let wake_tick = current_ticks + ms;
    let tid = current_tid();

    {
        let mut lock = SCHEDULER.lock();
        let sched = lock.as_mut().expect("Scheduler not initialized");
        if let Some(thread) = sched.threads.iter_mut().find(|t| t.id == tid) {
            thread.state = ThreadState::Sleeping(wake_tick);
        }
    }

    schedule();
}

/// Cooperatively yields the CPU to the next ready thread.
pub fn yield_now() {
    schedule();
}

/// Main scheduling routine. Selects next ready thread and performs context switch.
pub fn schedule() {
    if !is_initialized() {
        return;
    }

    let (old_rsp_ptr, new_rsp, next_rsp_top) = {
        let mut lock = SCHEDULER.lock();
        let sched = lock.as_mut().unwrap();

        let current_tid = sched.current_tid;

        // If current thread was running, put it back on the ready queue
        if let Some(curr) = sched.threads.iter_mut().find(|t| t.id == current_tid) {
            if curr.state == ThreadState::Running {
                curr.state = ThreadState::Ready;
                sched.ready_queue.push_back(current_tid);
            }
        }

        // Find next ready thread
        let mut next_tid = sched.idle_tid;
        while let Some(candidate_tid) = sched.ready_queue.pop_front() {
            if let Some(thread) = sched.threads.iter().find(|t| t.id == candidate_tid) {
                if thread.state == ThreadState::Ready {
                    next_tid = candidate_tid;
                    break;
                }
            }
        }

        if next_tid == current_tid {
            // Same thread, just continue
            if let Some(curr) = sched.threads.iter_mut().find(|t| t.id == current_tid) {
                curr.state = ThreadState::Running;
            }
            return;
        }

        // Prepare context switch
        let old_thread = sched.threads.iter_mut().find(|t| t.id == current_tid).unwrap();
        let old_rsp_ptr = &mut old_thread.rsp as *mut u64;

        let next_thread = sched.threads.iter_mut().find(|t| t.id == next_tid).unwrap();
        next_thread.state = ThreadState::Running;
        next_thread.quantum_remaining = 10;
        let new_rsp = next_thread.rsp;
        let next_rsp_top = next_thread.kernel_rsp_top;

        sched.current_tid = next_tid;
        CURRENT_TID.store(next_tid, Ordering::SeqCst);

        (old_rsp_ptr, new_rsp, next_rsp_top)
    };

    // Update TSS RSP0 and Syscall stack for user mode privilege transitions
    if next_rsp_top != 0 {
        crate::arch::x86_64::gdt::set_kernel_stack(next_rsp_top);
        crate::arch::x86_64::syscall::set_kernel_syscall_stack(next_rsp_top);
    }

    // Perform hardware context switch
    unsafe {
        context_switch(old_rsp_ptr, new_rsp);
    }
}

/// Called on every timer tick (1000 Hz / 1ms)
pub fn timer_tick() {
    if !is_initialized() {
        return;
    }

    let current_ticks = crate::drivers::timer::get_ticks();
    let mut should_reschedule = false;

    {
        let mut lock = match SCHEDULER.try_lock() {
            Some(g) => g,
            None => return, // Lock contended, skip tick
        };
        let sched = match lock.as_mut() {
            Some(s) => s,
            None => return,
        };

        // 1. Wake up sleeping threads
        for thread in sched.threads.iter_mut() {
            if let ThreadState::Sleeping(wake_tick) = thread.state {
                if current_ticks >= wake_tick {
                    thread.state = ThreadState::Ready;
                    sched.ready_queue.push_back(thread.id);
                }
            }
        }

        // 2. Decrement current thread quantum
        let curr_tid = sched.current_tid;
        if let Some(curr) = sched.threads.iter_mut().find(|t| t.id == curr_tid) {
            if curr.state == ThreadState::Running {
                if curr.quantum_remaining > 0 {
                    curr.quantum_remaining -= 1;
                }
                if curr.quantum_remaining == 0 {
                    should_reschedule = true;
                }
            }
        }
    }

    // If quantum expired, yield CPU
    if should_reschedule {
        schedule();
    }
}

pub fn list_threads() -> Vec<(usize, String, ThreadState, u8)> {
    let lock = SCHEDULER.lock();
    if let Some(sched) = lock.as_ref() {
        sched
            .threads
            .iter()
            .map(|t| (t.id, t.name.clone(), t.state, t.priority))
            .collect()
    } else {
        Vec::new()
    }
}
