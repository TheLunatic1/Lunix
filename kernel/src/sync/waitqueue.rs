use alloc::vec::Vec;
use spin::Mutex;

/// A WaitQueue allows threads to sleep waiting for an event or condition.
#[derive(Default)]
pub struct WaitQueue {
    waiters: Mutex<Vec<usize>>, // Thread IDs
}

impl WaitQueue {
    pub const fn new() -> Self {
        Self {
            waiters: Mutex::new(Vec::new()),
        }
    }

    /// Puts the current thread onto the waitqueue and blocks it.
    pub fn wait(&self) {
        let current_tid = crate::task::scheduler::current_tid();
        {
            let mut list = self.waiters.lock();
            if !list.contains(&current_tid) {
                list.push(current_tid);
            }
        }
        crate::task::scheduler::block_current_thread();
    }

    /// Wakes up a single waiting thread.
    pub fn wake_one(&self) -> bool {
        let tid_opt = {
            let mut list = self.waiters.lock();
            if list.is_empty() {
                None
            } else {
                Some(list.remove(0))
            }
        };

        if let Some(tid) = tid_opt {
            crate::task::scheduler::unblock_thread(tid);
            true
        } else {
            false
        }
    }

    /// Wakes up all waiting threads.
    pub fn wake_all(&self) -> usize {
        let tids = {
            let mut list = self.waiters.lock();
            let drained: Vec<usize> = list.drain(..).collect();
            drained
        };

        let count = tids.len();
        for tid in tids {
            crate::task::scheduler::unblock_thread(tid);
        }
        count
    }
}
