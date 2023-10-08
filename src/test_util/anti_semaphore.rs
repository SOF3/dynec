use std::time::Duration;

use parking_lot::{Condvar, Mutex};

/// A synchronization util that blocks until sufficiently many threads are waiting concurrently.
///
/// This is used for testing that multiple threads can run concurrently
/// (in contrast to one blocking the other).
#[derive(Debug)]
pub struct AntiSemaphore {
    saturation: usize,
    lock:       Mutex<AntiSemaphoreInner>,
    condvar:    Condvar,
}

#[derive(Debug)]
struct AntiSemaphoreInner {
    current: usize,
}

impl AntiSemaphore {
    /// Creates a new semaphore.
    /// `saturation` is the number of threads that can wait on the lock.
    pub fn new(saturation: usize) -> Self {
        Self {
            saturation,
            lock: Mutex::new(AntiSemaphoreInner { current: 0 }),
            condvar: Condvar::new(),
        }
    }

    /// Blocks until the semaphore is saturated.
    pub fn wait(&self) {
        let mut lock = self.lock.lock();
        log::trace!(
            "AntiSemaphore(current: {}, saturation: {}).wait()",
            lock.current,
            self.saturation
        );
        lock.current += 1;
        if lock.current > self.saturation {
            panic!("AntiSemaphore exceeded saturation");
        }

        if lock.current == self.saturation {
            lock.current = 0;
            self.condvar.notify_all();
        } else {
            let result = self.condvar.wait_for(&mut lock, Duration::from_secs(5));
            if result.timed_out() {
                panic!("Deadlock: AntiSemaphore not saturated for more than 5 seconds");
            }
        }
    }
}
