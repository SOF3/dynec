#![allow(missing_docs)]
#![allow(clippy::too_many_arguments)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::hash::Hash;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use indexmap::IndexSet;
use parking_lot::{Condvar, Mutex, Once};

use crate::entity::{self, ealloc};
use crate::{comp, global, storage, system, Archetype, Entity};

/// Records event and ensures that they are in the correct order.
pub struct EventTracer<T: fmt::Debug + Eq + Hash> {
    dependencies: HashMap<T, Vec<T>>,
    seen:         Mutex<IndexSet<T>>,
}

impl<T: fmt::Debug + Eq + Hash> EventTracer<T> {
    /// Creates a new event tracer that ensures `a` executes after `b` for each `(a, b)` input.
    pub fn new(orders: impl IntoIterator<Item = (T, T)>) -> Self {
        let mut dependencies: HashMap<T, Vec<T>> = HashMap::new();
        for (before, after) in orders {
            dependencies.entry(after).or_default().push(before);
        }
        let seen = Mutex::new(IndexSet::new());

        Self { dependencies, seen }
    }

    /// Records that `event` has happened.
    ///
    /// # Panics
    /// Panics if the same `event` was sent twice or a dependency is not satisfied.
    pub fn trace(&self, event: T) {
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

    /// Returns the events observed in this tracer.
    pub fn get_events(self) -> Vec<T> {
        let seen = self.seen.into_inner();
        seen.into_iter().collect()
    }
}

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

pub(crate) fn init() {
    static SET_LOGGER_ONCE: Once = Once::new();
    SET_LOGGER_ONCE.call_once(env_logger::init);
}

/// The default test archetype.
pub enum TestArch {}

impl Archetype for TestArch {
    type RawEntity = NonZeroU32;
    type Ealloc =
        ealloc::Recycling<NonZeroU32, BTreeSet<NonZeroU32>, ealloc::ThreadRngShardAssigner>;
}

/// A test discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec_codegen::Discrim)]
#[dynec(dynec_as(crate))]
pub struct TestDiscrim1(pub(crate) usize);

/// An alternative test discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dynec_codegen::Discrim)]
#[dynec(dynec_as(crate))]
pub struct TestDiscrim2(pub(crate) usize);

// Test component summary:
// Comp1: optional, depends []
// Comp2: optional, depends on Comp2
// Comp3: optional, depends on Comp1 and Comp2
// Comp4: optional, depends on Comp1 and Comp2
// Comp5: required, no init
// Comp6: required, depends []

/// optional, non-init, depless
#[comp(dynec_as(crate), of = TestArch)]
#[derive(Debug, PartialEq)]
pub struct Comp1(pub i32);

/// optional, depends on Comp1
#[comp(dynec_as(crate), of = TestArch, init = init_comp2/1)]
#[derive(Debug)]
pub struct Comp2(pub i32);
fn init_comp2(c1: &Comp1) -> Comp2 { Comp2(c1.0 + 2) }

/// optional, depends on Comp1 + Comp2
#[comp(
    dynec_as(crate),
    of = TestArch,
    init = |c1: &Comp1, c2: &Comp2| Comp3(c1.0 * 3, c2.0 * 5),
)]
#[derive(Debug)]
pub struct Comp3(pub i32, pub i32);

/// optional, depends on Comp1 + Comp2
#[comp(
    dynec_as(crate),
    of = TestArch,
    init = |c1: &Comp1, c2: &Comp2| Comp4(c1.0 * 7, c2.0 * 8),
)]
#[derive(Debug, PartialEq)]
pub struct Comp4(pub i32, pub i32);

/// required, non-init
#[comp(dynec_as(crate), of = TestArch, required)]
#[derive(Debug, PartialEq)]
pub struct Comp5(pub i32);

/// required, auto-init, depless
#[comp(dynec_as(crate), of = TestArch, required, init = || Comp6(9))]
#[derive(Debug)]
pub struct Comp6(pub i32);

/// non-init, has finalizers
#[comp(dynec_as(crate), of = TestArch, finalizer)]
pub struct CompFinal;

/// a generic component
pub struct CompN<const N: usize>(pub i32);

impl<const N: usize> entity::Referrer for CompN<N> {
    fn visit_type(arg: &mut entity::referrer::VisitTypeArg) { arg.mark::<Self>(); }
    fn visit_mut<V: entity::referrer::VisitMutArg>(&mut self, _: &mut V) {}
}

impl<const N: usize> comp::SimpleOrIsotope<TestArch> for CompN<N> {
    const PRESENCE: comp::Presence = comp::Presence::Optional;
    const INIT_STRATEGY: comp::InitStrategy<TestArch, Self> = comp::InitStrategy::None;

    type Storage = storage::Vec<NonZeroU32, Self>;
}
impl<const N: usize> comp::Simple<TestArch> for CompN<N> {
    const IS_FINALIZER: bool = false;
}

/// Does not have auto init
#[comp(dynec_as(crate), of = TestArch, isotope = TestDiscrim1)]
#[derive(Debug, Clone, PartialEq)]
pub struct Iso1(pub i32);

/// Has auto init
#[comp(dynec_as(crate), of = TestArch, isotope = TestDiscrim2)]
#[derive(Debug, Clone, PartialEq)]
pub struct Iso2(pub i32);

/// A simple component with a strong reference to [`TestArch`].
#[comp(dynec_as(crate), of = TestArch)]
pub struct StrongRefSimple(#[entity] pub Entity<TestArch>);

/// An isotope component with a strong reference to [`TestArch`].
#[comp(dynec_as(crate), of = TestArch, isotope = TestDiscrim1)]
pub struct StrongRefIsotope(#[entity] pub Entity<TestArch>);

/// A generic global state with an initializer.
#[global(dynec_as(crate), initial)]
#[derive(Default)]
pub struct Aggregator {
    pub comp30_sum:     i32,
    pub comp41_product: i32,
}

/// An entity-referencing global state.
#[global(dynec_as(crate), initial)]
#[derive(Default)]
pub struct InitialEntities {
    /// A strong reference.
    #[entity]
    pub strong: Option<Entity<TestArch>>,
    /// A weak reference.
    #[entity]
    pub weak:   Option<entity::Weak<TestArch>>,
}

/// A dummy system used for registering all non-entity-referencing test components.
#[system(dynec_as(crate))]
pub fn use_all_bare(
    _comp1: impl system::ReadSimple<TestArch, Comp1>,
    _comp2: impl system::ReadSimple<TestArch, Comp2>,
    _comp3: impl system::ReadSimple<TestArch, Comp3>,
    _comp4: impl system::ReadSimple<TestArch, Comp4>,
    _comp5: impl system::ReadSimple<TestArch, Comp5>,
    _comp6: impl system::ReadSimple<TestArch, Comp6>,
    _comp_final: impl system::ReadSimple<TestArch, CompFinal>,
    _iso1: impl system::ReadIsotope<TestArch, Iso1>,
    _iso2: impl system::ReadIsotope<TestArch, Iso2>,
    #[dynec(global)] _agg: &Aggregator,
) {
}

/// A dummy system with minimally simple dependencies.
#[system(dynec_as(crate))]
pub fn use_comp_n(
    _comp0: impl system::ReadSimple<TestArch, CompN<0>>,
    _comp1: impl system::ReadSimple<TestArch, CompN<1>>,
    _comp2: impl system::ReadSimple<TestArch, CompN<2>>,
    _comp3: impl system::ReadSimple<TestArch, CompN<3>>,
    _comp4: impl system::ReadSimple<TestArch, CompN<4>>,
    _comp5: impl system::ReadSimple<TestArch, CompN<5>>,
    _comp6: impl system::ReadSimple<TestArch, CompN<6>>,
    _comp7: impl system::ReadSimple<TestArch, CompN<7>>,
    _comp8: impl system::ReadSimple<TestArch, CompN<8>>,
    _comp9: impl system::ReadSimple<TestArch, CompN<9>>,
    _comp10: impl system::ReadSimple<TestArch, CompN<10>>,
    _comp11: impl system::ReadSimple<TestArch, CompN<11>>,
    _comp12: impl system::ReadSimple<TestArch, CompN<12>>,
    _comp13: impl system::ReadSimple<TestArch, CompN<13>>,
    _comp14: impl system::ReadSimple<TestArch, CompN<14>>,
    _comp15: impl system::ReadSimple<TestArch, CompN<15>>,
) {
}
