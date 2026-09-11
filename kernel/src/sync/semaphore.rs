use crate::sync::waitqueue::WaitQueue;
use core::sync::atomic::{AtomicIsize, Ordering};

/// A counting Semaphore synchronization primitive.
pub struct Semaphore {
    count: AtomicIsize,
    wait_queue: WaitQueue,
}

impl Semaphore {
    pub const fn new(initial: isize) -> Self {
        Self {
            count: AtomicIsize::new(initial),
            wait_queue: WaitQueue::new(),
        }
    }

    /// Acquires a permit from the semaphore, blocking if count <= 0.
    pub fn acquire(&self) {
        loop {
            let current = self.count.load(Ordering::Acquire);
            if current > 0 {
                if self
                    .count
                    .compare_exchange_weak(current, current - 1, Ordering::AcqRel, Ordering::Relaxed)
                    .is_ok()
                {
                    break;
                }
            } else {
                // Wait on queue
                self.wait_queue.wait();
            }
        }
    }

    /// Releases a permit back to the semaphore, waking up any waiting thread.
    pub fn release(&self) {
        self.count.fetch_add(1, Ordering::Release);
        self.wait_queue.wake_one();
    }

    /// Returns current available permit count.
    pub fn count(&self) -> isize {
        self.count.load(Ordering::Relaxed)
    }
}
