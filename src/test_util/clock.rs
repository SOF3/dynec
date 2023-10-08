use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};

/// An emulated clock that supports ticking.
pub struct Clock<T: Tick> {
    inner:              Mutex<Inner<T>>,
    check_completeness: Condvar,
}

struct Inner<T: Tick> {
    iter: T::Iterator,
    now:  T,
    map:  BTreeMap<T, Arc<Condvar>>,
}

impl<T: Tick> Default for Clock<T> {
    fn default() -> Self {
        let mut iter = T::iter();
        let now = iter.next().expect("Tick enum must not be empty");
        Self {
            inner:              Mutex::new(Inner { iter, now, map: BTreeMap::new() }),
            check_completeness: Condvar::new(),
        }
    }
}

impl<T: Tick> Clock<T>
where
    T::Iterator: Send + Sync,
{
    /// Blocks the thread until the clock ticks `until`.
    ///
    /// Asserts the current tick is `now`.
    pub fn wait(&self, now: T, until: T) {
        let mut inner = self.inner.lock();

        assert!(now < until);
        assert!(now == inner.now);

        let cv = Arc::clone(inner.map.entry(until).or_default());
        cv.wait(&mut inner);

        self.check_completeness.notify_one();
    }

    /// Sets the clock to the next tick.
    ///
    /// Asserts the current tick is `expect`.
    pub(crate) fn tick(&self, expect: T) {
        let mut inner = self.inner.lock();

        let next = inner.iter.next().expect("Tick enum has been exhausted");
        assert!(next == expect);

        inner.now = next;

        if let Some(cv) = inner.map.get(&next) {
            cv.notify_all();
        }
    }

    /// Orchestrates a test with this clock.
    pub(crate) fn orchestrate(&self, mut can_tick_complete: impl FnMut(T) -> bool + Send) {
        rayon::scope(|scope| {
            scope.spawn(|_| {
                let mut inner = self.inner.lock();

                for (i, tick) in T::iter().enumerate() {
                    if i > 0 {
                        self.tick(tick);
                    }

                    let timeout = Instant::now() + Duration::from_secs(5);

                    loop {
                        if can_tick_complete(tick) {
                            break;
                        } else {
                            if timeout < Instant::now() {
                                panic!(
                                    "Timeout exceeded without fulfilling completeness \
                                     requirements of {:?}",
                                    tick
                                );
                            }

                            self.check_completeness.wait_until(&mut inner, timeout);
                        }
                    }
                }
            });
        });
    }
}

pub trait Tick:
    fmt::Debug + Copy + Eq + Ord + strum::IntoEnumIterator + Send + Sync + Sized
{
}
impl<T: fmt::Debug + Copy + Eq + Ord + strum::IntoEnumIterator + Send + Sync + Sized> Tick for T {}
