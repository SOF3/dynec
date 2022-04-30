use std::collections::BTreeSet;

use crate::entity;

/// Manages entity ID allocation.
pub(crate) struct Ealloc {
    /// The next ID to allocate if there is nothing to recycle.
    gauge:    entity::Raw,
    /// The set of freed entity IDs.
    recycled: BTreeSet<entity::Raw>,
}

impl Default for Ealloc {
    fn default() -> Self { Self { gauge: entity::Raw::smallest(), recycled: BTreeSet::new() } }
}

impl Ealloc {
    /// Allocates the smallest entity ID available.
    pub(crate) fn allocate(&mut self) -> entity::Raw {
        // TODO change to pop_first when it is stable
        if let Some(&id) = self.recycled.iter().next() {
            self.recycled.remove(&id);
            id
        } else {
            self.push_gauge()
        }
    }

    /// Allocates an entity ID.
    pub(crate) fn allocate_near(&mut self, hint: entity::Raw) -> entity::Raw {
        let mut left = self.recycled.range(..hint);
        let mut right = self.recycled.range(hint..);

        let selected = match (left.next(), right.next()) {
            (Some(left), Some(right)) => {
                let hint_int = hint.0.get();

                let left_int = left.0.get();
                let left_delta = hint_int - left_int;

                let right_int = right.0.get();
                let right_delta = right_int - hint_int;

                Some(if left_delta < right_delta { left } else { right })
            }
            (Some(left), None) => Some(left),
            (None, Some(right)) => Some(right),
            (None, None) => None,
        }
        .copied();

        if let Some(selected) = selected {
            let removed = self.recycled.remove(&selected);
            if !removed {
                panic!("Logic error: cannot consume recycled entity ID");
            }
            selected
        } else {
            self.push_gauge()
        }
    }

    /// Pushes the gauge and returns the newly allocated value.
    fn push_gauge(&mut self) -> entity::Raw {
        let next = self.gauge;
        self.gauge = self.gauge.increment();
        next
    }

    /// Frees an entity ID.
    pub(crate) fn free(&mut self, value: entity::Raw) {
        let new = self.recycled.insert(value);
        if !new {
            panic!("An entity is freed more than once");
        }
    }
}
