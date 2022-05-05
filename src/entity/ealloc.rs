use std::collections::BTreeSet;

use crate::{entity, util};

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
        if let Some(id) = util::btreeset_remove_first(&mut self.recycled) {
            id
        } else {
            self.push_gauge()
        }
    }

    /// Allocates an entity ID.
    pub(crate) fn allocate_near(&mut self, hint: entity::Raw) -> entity::Raw {
        let mut left = self.recycled.range(..hint).rev();
        let mut right = self.recycled.range(hint..);

        let selected = match (left.next(), right.next()) {
            (Some(left), Some(right)) => {
                let hint_int = hint.0.get();

                let left_int = left.0.get();
                let left_delta = hint_int - left_int;

                let right_int = right.0.get();
                let right_delta = right_int - hint_int;

                Some(if left_delta <= right_delta { left } else { right })
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

#[cfg(test)]
mod tests {
    use super::Ealloc;

    #[test]
    fn test_realloc_freed() {
        let mut ealloc = Ealloc::default();
        let r1 = ealloc.allocate();
        let r2 = ealloc.allocate();
        let r3 = ealloc.allocate();

        assert_eq!(r1.0.get(), 1);
        assert_eq!(r2.0.get(), 2);
        assert_eq!(r3.0.get(), 3);

        ealloc.free(r2);

        let r4 = ealloc.allocate();
        let r5 = ealloc.allocate();

        assert_eq!(r4.0.get(), 2);
        assert_eq!(r5.0.get(), 4);

        ealloc.free(r3);
        ealloc.free(r5);

        let r6 = ealloc.allocate();
        let r7 = ealloc.allocate();
        let r8 = ealloc.allocate();

        assert_eq!(r6.0.get(), 3);
        assert_eq!(r7.0.get(), 4);
        assert_eq!(r8.0.get(), 5);
    }

    #[test]
    fn test_realloc_near() {
        let mut ealloc = Ealloc::default();

        let r1 = ealloc.allocate();
        let r2 = ealloc.allocate();
        let r3 = ealloc.allocate();
        let r4 = ealloc.allocate();

        ealloc.free(r2);
        ealloc.free(r3);

        let r5 = ealloc.allocate_near(r4);
        let r6 = ealloc.allocate_near(r4);

        assert_eq!(r5.0.get(), 3);
        assert_eq!(r6.0.get(), 2);

        ealloc.free(r5);
        ealloc.free(r6);

        let r7 = ealloc.allocate_near(r1);
        let r8 = ealloc.allocate_near(r1);

        assert_eq!(r7.0.get(), 2);
        assert_eq!(r8.0.get(), 3);
    }
}
