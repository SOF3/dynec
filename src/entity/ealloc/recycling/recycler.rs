use std::collections::BTreeSet;

use crate::entity::Raw;

/// A data structure that provides the ability to recycle entity IDs.
pub trait Recycler<E: Raw>: Default + Extend<E> + Send + 'static {
    /// Additional configuration for polling.
    type Hint: Default;

    /// Returns the length of this recycler.
    fn len(&self) -> usize;

    /// Returns whether the recycler is empty.
    fn is_empty(&self) -> bool { self.len() == 0 }

    /// Polls an ID from the recycler based on the given hint.
    fn poll(&mut self, hint: Self::Hint) -> Option<E>;
}

/// A minimal allocator implemented through a FILO stack.
impl<E: Raw> Recycler<E> for Vec<E> {
    type Hint = ();

    fn len(&self) -> usize { Vec::len(self) }

    fn poll(&mut self, (): ()) -> Option<E> { self.pop() }
}

/// Additional configuration for allocating entities from a BTreeSet recycler.
pub struct BTreeHint<R> {
    /// Try to allocate the entity somewhere nearest to the given value.
    pub near: Option<R>,
}

impl<E: Raw> Default for BTreeHint<E> {
    fn default() -> Self { Self { near: None } }
}

impl<E: Raw> Recycler<E> for BTreeSet<E> {
    type Hint = BTreeHint<E>;

    fn len(&self) -> usize { BTreeSet::len(self) }

    fn poll(&mut self, hint: Self::Hint) -> Option<E> {
        if let Some(near) = hint.near {
            let mut left = self.range(..near).rev();
            let mut right = self.range(near..);

            let selected = match (left.next(), right.next()) {
                (Some(&left), Some(&right)) => {
                    let left_delta = near.sub(left);
                    let right_delta = right.sub(near);
                    Some(if left_delta <= right_delta { left } else { right })
                }
                (Some(&left), None) => Some(left),
                (None, Some(&right)) => Some(right),
                (None, None) => None,
            };

            if let Some(selected) = selected {
                let removed = self.remove(&selected);
                if !removed {
                    panic!("self.range() item is not in self");
                }
                Some(selected)
            } else {
                None
            }
        } else {
            self.pop_first()
        }
    }
}
