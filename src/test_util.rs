#![allow(dead_code)] // TODO remove when tests are more comprehensive

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::hash::Hash;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use indexmap::IndexSet;
use parking_lot::{Condvar, Mutex, Once};

use crate::entity::ealloc;
use crate::Archetype;

/// Records event and ensures that they are in the correct order.
pub(crate) struct EventTracer<T: fmt::Debug + Eq + Hash> {
    dependencies: HashMap<T, Vec<T>>,
    seen:         Mutex<IndexSet<T>>,
}

impl<T: fmt::Debug + Eq + Hash> EventTracer<T> {
    pub(crate) fn new(orders: impl IntoIterator<Item = (T, T)>) -> Self {
        let mut dependencies: HashMap<T, Vec<T>> = HashMap::new();
        for (before, after) in orders {
            dependencies.entry(after).or_default().push(before);
        }
        let seen = Mutex::new(IndexSet::new());

        Self { dependencies, seen }
    }

    pub(crate) fn trace(&self, event: T) {
        let mut seen = self.seen.lock();

        if let Some(deps) = self.dependencies.get(&event) {
            for dep in deps {
                assert!(seen.contains(dep), "{:?} should happen after {:?}", event, dep);
            }
        }

        let (index, new) = seen.insert_full(event);
        assert!(
            !new,
            "{:?} is inserted twice",
            seen.get_index(index).expect("insert_full should return valid index")
        );
    }

    pub(crate) fn get_events(self) -> Vec<T> {
        let seen = self.seen.into_inner();
        seen.into_iter().collect()
    }
}

/// An emulated clock that supports ticking.
pub(crate) struct Clock<T: Tick> {
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
    pub(crate) fn wait(&self, now: T, until: T) {
        let mut inner = self.inner.lock();

        assert!(now < until);
        assert!(now == inner.now);

        let cv = Arc::clone(inner.map.entry(until).or_default());
        cv.wait(&mut inner);

        self.check_completeness.notify_one();
    }

    pub(crate) fn tick(&self, expect: T) {
        let mut inner = self.inner.lock();

        let next = inner.iter.next().expect("Tick enum has been exhausted");
        assert!(next == expect);

        inner.now = next;

        if let Some(cv) = inner.map.get(&next) {
            cv.notify_all();
        }
    }

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

pub(crate) trait Tick:
    fmt::Debug + Copy + Eq + Ord + strum::IntoEnumIterator + Send + Sync + Sized
{
}
impl<T: fmt::Debug + Copy + Eq + Ord + strum::IntoEnumIterator + Send + Sync + Sized> Tick for T {}

#[derive(Debug)]
pub(crate) struct AntiSemaphore {
    saturation: usize,
    lock:       Mutex<AntiSemaphoreInner>,
    condvar:    Condvar,
}

#[derive(Debug)]
struct AntiSemaphoreInner {
    current: usize,
}

impl AntiSemaphore {
    pub(crate) fn new(saturation: usize) -> Self {
        Self {
            saturation,
            lock: Mutex::new(AntiSemaphoreInner { current: 0 }),
            condvar: Condvar::new(),
        }
    }

    pub(crate) fn wait(&self) {
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

pub(crate) enum TestArch {}

impl Archetype for TestArch {
    type RawEntity = NonZeroU32;
    type Ealloc =
        ealloc::Recycling<NonZeroU32, BTreeSet<NonZeroU32>, ealloc::ThreadRngShardAssigner>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec_codegen::Discrim)]
#[dynec(dynec_as(crate))]
pub(crate) struct TestDiscrim1(pub(crate) usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec_codegen::Discrim)]
#[dynec(dynec_as(crate))]
pub(crate) struct TestDiscrim2(pub(crate) usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec_codegen::Discrim)]
#[dynec(dynec_as(crate))]
pub(crate) struct TestDiscrim3(pub(crate) usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec_codegen::Discrim)]
#[dynec(dynec_as(crate))]
pub(crate) struct TestDiscrim4(pub(crate) usize);

pub(crate) fn init() {
    static SET_LOGGER_ONCE: Once = Once::new();
    SET_LOGGER_ONCE.call_once(env_logger::init);
}
