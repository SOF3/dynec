use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;

use indexmap::IndexSet;
use parking_lot::Mutex;

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
