use core::fmt;
use std::any::TypeId;
use std::collections::{BTreeSet, BinaryHeap};
use std::hash;
use std::iter::Peekable;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::Archetype;

pub(crate) type RawId = u32;

#[derive(Default)]
pub(crate) struct Counter {
    top: RawId,
    gaps: BTreeSet<RawId>,
}

impl Counter {
    /// Allocates an entity ID.
    ///
    /// If there are deleted entities near the `near` parameter,
    /// the new entity ID is allocated using the closest gap possible.
    /// Otherwise, a new entity is allocated.
    pub(crate) fn allocate(&mut self, near: Option<RawId>) -> RawId {
        let choice = match near {
            Some(near) => {
                let left = self.gaps.range(..near).next();
                let right = self.gaps.range(near..).next();

                match (left, right) {
                    (Some(&left), Some(&right)) => {
                        let left_delta = near - left;
                        let right_delta = right - near;

                        let choice = if left_delta < right_delta {
                            left
                        } else {
                            right
                        };

                        Some(choice)
                    }
                    (Some(&left), None) => Some(left),
                    (None, Some(&right)) => Some(right),
                    (None, None) => None,
                }
            }
            None => self.gaps.iter().copied().next(),
        };

        if let Some(choice) = choice {
            let present = self.gaps.remove(&choice);
            debug_assert!(present);
            choice
        } else {
            let ret = self.top;
            self.top += 1;
            ret
        }
    }

    /// Frees an unused entity ID.
    pub(crate) fn delete(&mut self, id: RawId) {
        let new = self.gaps.insert(id);
        assert!(new, "Entity is deleted twice")
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = RawId> + '_ {
        struct Iter<I1, I2>(I1, I2);

        impl<'t, I1, I2> Iterator for Iter<I1, Peekable<I2>>
        where
            I1: Iterator<Item = RawId>,
            I2: Iterator<Item = RawId> + 't,
        {
            type Item = RawId;

            fn next(&mut self) -> Option<Self::Item> {
                while let Some(id) = self.0.next() {
                    if self.1.peek() == Some(&id) {
                        self.1.next();
                        continue;
                    }
                    return Some(id);
                }
                None
            }
        }

        Iter(0..self.top, self.gaps.iter().copied().peekable())
    }
}

pub trait Ref<A: Archetype>: RefInternal<A> {}

mod internal {
    use super::*;

    pub trait RefInternal<A: Archetype> {
        fn id(&self) -> RawId;
    }
}

pub(crate) use internal::RefInternal;

/// References an entity.
pub struct Entity<A: Archetype> {
    pub(crate) id: RawId,
    /// Reference counter of the entity.
    arc: Arc<()>,
    _ph: PhantomData<&'static A>,
}

impl<A: Archetype> Entity<A> {
    /// Creates a mutable reference to the entity ID.
    pub fn to_runtime_ref(&mut self) -> RuntimeEntityRef<'_> {
        RuntimeEntityRef {
            ty: TypeId::of::<A>(),
            id: &mut self.id,
        }
    }

    /// Returns the number of references to the entity.
    pub fn ref_count(&self) -> usize {
        Arc::strong_count(&self.arc)
    }

    /// Creates a weak reference to this entity.
    ///
    /// This is useful for tracking entity when it is getting deleted.
    pub fn weak(&self) -> Weak<A> {
        Weak {
            id: self.id,
            _ph: PhantomData,
        }
    }
}

impl<A: Archetype> fmt::Debug for Entity<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entity")
            .field("id", &self.id)
            .field("refcount", &Arc::strong_count(&self.arc))
            .finish()
    }
}

impl<A: Archetype> Clone for Entity<A> {
    fn clone(&self) -> Self {
        Entity {
            id: self.id,
            arc: Arc::clone(&self.arc),
            _ph: PhantomData,
        }
    }
}

impl<A: Archetype> PartialEq for Entity<A> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<A: Archetype> Eq for Entity<A> {}

impl<A: Archetype> PartialOrd for Entity<A> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<A: Archetype> Ord for Entity<A> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl<A: Archetype> hash::Hash for Entity<A> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<A: Archetype> RefInternal<A> for Entity<A> {
    fn id(&self) -> RawId {
        self.id
    }
}
impl<A: Archetype> Ref<A> for Entity<A> {}

/// A weak reference to an entity.
pub struct Weak<A: Archetype> {
    pub(crate) id: RawId,
    _ph: PhantomData<&'static A>,
}

impl<A: Archetype> fmt::Debug for Weak<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("entity::Weak")
            .field("id", &self.id)
            .finish()
    }
}

impl<A: Archetype> Clone for Weak<A> {
    fn clone(&self) -> Self {
        Weak {
            id: self.id,
            _ph: PhantomData,
        }
    }
}

impl<A: Archetype> Copy for Weak<A> {}

impl<A: Archetype> PartialEq<Weak<A>> for Entity<A> {
    fn eq(&self, other: &Weak<A>) -> bool {
        self.id == other.id
    }
}

impl<A: Archetype> PartialEq<Entity<A>> for Weak<A> {
    fn eq(&self, other: &Entity<A>) -> bool {
        self.id == other.id
    }
}

impl<A: Archetype> RefInternal<A> for Weak<A> {
    fn id(&self) -> RawId {
        self.id
    }
}
impl<A: Archetype> Ref<A> for Weak<A> {}

/// A mutable reference to an [`Entity`] with archetype as a runtime value.
pub struct RuntimeEntityRef<'t> {
    pub(crate) ty: TypeId,
    pub(crate) id: &'t mut RawId,
}

impl<A: Archetype> Entity<A> {
    /// Constructs an entity ID from the raw ID.
    pub(crate) fn new(id: RawId) -> Self {
        Self {
            id,
            arc: Arc::new(()),
            _ph: PhantomData,
        }
    }
}

/// Implemented by types that own [`Entity`] transitively.
/// Archetype permutation panics if this trait is not implemented correctly.
pub trait Watcher {
    /// Executes the closure to mutate each owned [`Entity`].
    ///
    /// This is used to update entity references after permutation takes place.
    fn for_each_entity<'t>(&'t mut self, f: &dyn Fn(RuntimeEntityRef<'t>));
}
