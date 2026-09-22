use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use spin::Mutex;

use crate::task::process::Process;
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
pub static PROCESS_TABLE: Mutex<BTreeMap<usize, Arc<Mutex<Process>>>> = Mutex::new(BTreeMap::new());
static NEXT_PID: AtomicUsize = AtomicUsize::new(2);

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
        fs_base: 0,
        entry_fn: None,
        is_user: false,
        fpu: super::thread::FpuState::initial(),
        clear_child_tid: 0,
    };
    sched.threads.push(main_thread);

    // 2. Create idle thread (TID 1)
    let idle_thread = Thread::new_idle(1);
    sched.threads.push(idle_thread);

    *SCHEDULER.lock() = Some(sched);
    CURRENT_TID.store(0, Ordering::SeqCst);
    SCHEDULER_INITIALIZED.store(true, Ordering::SeqCst);

    // Register initial kernel process (PID 0)
    let kernel_proc = Arc::new(Mutex::new(Process::new_kernel()));
    PROCESS_TABLE.lock().insert(0, kernel_proc);
}

pub fn allocate_pid() -> usize {
    NEXT_PID.fetch_add(1, Ordering::SeqCst)
}

pub fn register_process(proc: Process) -> usize {
    let pid = proc.id;
    let proc_arc = Arc::new(Mutex::new(proc));
    PROCESS_TABLE.lock().insert(pid, proc_arc);
    pid
}

pub fn get_process(pid: usize) -> Option<Arc<Mutex<Process>>> {
    PROCESS_TABLE.lock().get(&pid).cloned()
}

pub fn get_current_process() -> Option<Arc<Mutex<Process>>> {
    let tid = current_tid();
    let pid = {
        let lock = SCHEDULER.lock();
        let sched = lock.as_ref()?;
        sched.threads.iter().find(|t| t.id == tid).map(|t| t.process_id).unwrap_or(0)
    };
    get_process(pid)
}

pub fn current_pid() -> usize {
    let tid = current_tid();
    let lock = SCHEDULER.lock();
    if let Some(sched) = lock.as_ref() {
        sched.threads.iter().find(|t| t.id == tid).map(|t| t.process_id).unwrap_or(0)
    } else {
        0
    }
}

/// True if another live process (a `vfork` parent) still uses address space `cr3`.
pub fn address_space_shared(cr3: u64, except_pid: usize) -> bool {
    PROCESS_TABLE.lock().iter().any(|(&pid, p)| {
        pid != except_pid
            && p.try_lock().map_or(true, |g| g.is_alive && g.cr3 == cr3)
    })
}

pub fn set_process_exit_code(pid: usize, code: i32) {
    if let Some(proc_arc) = get_process(pid) {
        let mut proc = proc_arc.lock();
        proc.is_alive = false;
        proc.exit_code = Some(code);
    }
}

pub fn reap_child_process(parent_pid: usize, target_child: isize) -> Option<(usize, i32)> {
    let mut proc_table = PROCESS_TABLE.lock();
    let parent_arc = proc_table.get(&parent_pid)?.clone();
    let mut parent = parent_arc.lock();

    let mut terminated_idx = None;
    let mut result = None;

    for (idx, &child_pid) in parent.children.iter().enumerate() {
        if target_child == -1 || (target_child > 0 && child_pid == target_child as usize) {
            if let Some(child_arc) = proc_table.get(&child_pid) {
                let child = child_arc.lock();
                if !child.is_alive {
                    let code = child.exit_code.unwrap_or(0);
                    result = Some((child_pid, code));
                    terminated_idx = Some(idx);
                    break;
                }
            }
        }
    }

    if let Some(idx) = terminated_idx {
        let (reaped_pid, _) = result.unwrap();
        parent.children.remove(idx);
        proc_table.remove(&reaped_pid);
    }

    result
}

pub fn spawn_with_pid(name: &str, pid: usize, entry: fn(), priority: u8) -> usize {
    let mut lock = SCHEDULER.lock();
    let sched = lock.as_mut().expect("Scheduler not initialized");

    let tid = sched.next_tid;
    sched.next_tid += 1;

    let mut thread = Thread::new_kernel(tid, name, entry, priority);
    thread.process_id = pid;
    sched.threads.push(thread);
    sched.ready_queue.push_back(tid);

    tid
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

    let (old_rsp_ptr, new_rsp, next_rsp_top, next_pid, next_fs_base, old_fpu, next_fpu) = {
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
        unsafe {
            old_thread.fs_base = crate::arch::x86_64::io::rdmsr(0xC000_0100);
        }
        let old_rsp_ptr = &mut old_thread.rsp as *mut u64;
        let old_fpu = old_thread.fpu.0.as_mut_ptr();

        let next_thread = sched.threads.iter_mut().find(|t| t.id == next_tid).unwrap();
        next_thread.state = ThreadState::Running;
        next_thread.quantum_remaining = 10;
        let new_rsp = next_thread.rsp;
        let next_rsp_top = next_thread.kernel_rsp_top;
        let next_fs_base = next_thread.fs_base;

        let next_pid = next_thread.process_id;
        let next_fpu = next_thread.fpu.0.as_ptr();

        sched.current_tid = next_tid;
        CURRENT_TID.store(next_tid, Ordering::SeqCst);

        (old_rsp_ptr, new_rsp, next_rsp_top, next_pid, next_fs_base, old_fpu, next_fpu)
    };

    // Update TSS RSP0 and Syscall stack for user mode privilege transitions
    if next_rsp_top != 0 {
        crate::arch::x86_64::gdt::set_kernel_stack(next_rsp_top);
        crate::arch::x86_64::syscall::set_kernel_syscall_stack(next_rsp_top);
    }

    // Restore incoming thread's TLS FS_BASE MSR (0xC000_0100)
    unsafe {
        crate::arch::x86_64::io::wrmsr(0xC000_0100, next_fs_base);
    }

    // Switch CR3 to target process's page table if different
    if let Some(proc_arc) = get_process(next_pid) {
        let proc_cr3 = proc_arc.lock().cr3;
        if proc_cr3 != 0 {
            let (current_cr3, flags) = x86_64::registers::control::Cr3::read();
            if current_cr3.start_address().as_u64() != proc_cr3 {
                unsafe {
                    x86_64::registers::control::Cr3::write(
                        x86_64::structures::paging::PhysFrame::containing_address(x86_64::PhysAddr::new(proc_cr3)),
                        flags,
                    );
                }
            }
        }
    }

    // Floating point / SSE registers belong to the thread: save ours, load the next one's.
    // (The kernel itself is built without SSE, so nothing has touched them in between.)
    unsafe {
        core::arch::asm!("fxsave64 [{}]", in(reg) old_fpu, options(nostack, preserves_flags));
        core::arch::asm!("fxrstor64 [{}]", in(reg) next_fpu, options(nostack, preserves_flags));
    }

    // Perform hardware context switch
    unsafe {
        context_switch(old_rsp_ptr, new_rsp);
    }
}

pub fn set_thread_clear_child_tid(tid: usize, ptr: u64) {
    if let Some(sched) = SCHEDULER.lock().as_mut() {
        if let Some(t) = sched.threads.iter_mut().find(|t| t.id == tid) {
            t.clear_child_tid = ptr;
        }
    }
}

/// Take (and clear) the current thread's clear_child_tid pointer.
pub fn take_current_clear_child_tid() -> u64 {
    let tid = current_tid();
    let mut lock = SCHEDULER.lock();
    lock.as_mut()
        .and_then(|s| s.threads.iter_mut().find(|t| t.id == tid))
        .map(|t| core::mem::take(&mut t.clear_child_tid))
        .unwrap_or(0)
}

/// Number of threads of process `pid` that have not exited.
pub fn live_thread_count(pid: usize) -> usize {
    SCHEDULER
        .lock()
        .as_ref()
        .map(|s| s.threads.iter().filter(|t| t.process_id == pid && t.state != ThreadState::Dead).count())
        .unwrap_or(0)
}

/// `exit_group`: terminate every thread of `pid` except `keep_tid`.
pub fn kill_other_threads(pid: usize, keep_tid: usize) {
    if let Some(sched) = SCHEDULER.lock().as_mut() {
        for t in sched.threads.iter_mut() {
            if t.process_id == pid && t.id != keep_tid && pid != 0 {
                t.state = ThreadState::Dead;
            }
        }
    }
}

/// Give thread `tid` a copy of the calling thread's current FPU/SSE registers (fork).
pub fn inherit_fpu_state(tid: usize) {
    let mut lock = SCHEDULER.lock();
    if let Some(sched) = lock.as_mut() {
        if let Some(thread) = sched.threads.iter_mut().find(|t| t.id == tid) {
            unsafe {
                core::arch::asm!("fxsave64 [{}]", in(reg) thread.fpu.0.as_mut_ptr(), options(nostack, preserves_flags));
            }
        }
    }
}

pub fn set_thread_fs_base(tid: usize, fs_base: u64) {
    let mut lock = SCHEDULER.lock();
    if let Some(sched) = lock.as_mut() {
        if let Some(thread) = sched.threads.iter_mut().find(|t| t.id == tid) {
            thread.fs_base = fs_base;
        }
    }
}

pub fn set_current_thread_fs_base(fs_base: u64) {
    let tid = current_tid();
    set_thread_fs_base(tid, fs_base);
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
