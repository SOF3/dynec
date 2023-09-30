//! Manages entity ID allocation and deallocation.

use std::any::{Any, TypeId};
use std::cell::{self, RefCell};
use std::collections::HashMap;
use std::{iter, ops};

use super::raw::Raw;
use crate::entity::raw::Atomic;
use crate::util::DbgTypeId;
use crate::Archetype;

mod recycling;
pub use recycling::{BTreeHint, Recycler, Recycling, RecyclingShard};

mod sharding;
pub(crate) use sharding::AnyShard;
pub use sharding::{Shard, ShardAssigner, StaticShardAssigner, ThreadRngShardAssigner};

pub(crate) mod snapshot;
pub use snapshot::Snapshot;

pub(crate) type AnyBuilder = Box<dyn FnOnce(usize) -> Box<dyn AnyEalloc>>;

pub(crate) fn builder<A: Archetype>() -> AnyBuilder {
    Box::new(|num_shards| Box::new(A::Ealloc::new(num_shards)) as Box<dyn AnyEalloc>)
}

/// Manages sharded entity ID allocation and deallocation.
pub trait Ealloc: 'static {
    /// The raw entity ID type supported by this allocator.
    type Raw: Raw;

    /// The hint type supported by the allocator to fine-tune memory allocation.
    type AllocHint: Default;

    /// The shard type sent to each worker thread.
    type Shard: Shard<Raw = Self::Raw, Hint = Self::AllocHint>;

    /// Initialize a new allocator with `num_shards` shards.
    ///
    /// `num_shards` is always nonzero.
    fn new(num_shards: usize) -> Self;

    /// Populates `vec` with the transformed shards.
    ///
    /// The length of the `vec` must be `num_shards` in [`Ealloc::new`].
    /// The implementation shall shuffle the results returned by this method
    /// to avoid centralizing on a single shard.
    fn shards<U, F: Fn(Self::Shard) -> U>(&mut self, vec: &mut Vec<U>, transform: F);

    /// Takes a snapshot of the available entity IDs.
    fn snapshot(&self) -> Snapshot<Self::Raw>;

    /// Allocate an ID in offline mode.
    fn allocate(&mut self, hint: Self::AllocHint) -> Self::Raw;

    /// Queues the deallocation of an ID.
    fn queue_deallocate(&mut self, id: Self::Raw);

    /// Flushes the queued operations after joining.
    fn flush(&mut self);
}

pub(crate) trait AnyEalloc {
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn shards(&mut self, vec: &mut Vec<Box<dyn AnyShard>>);

    fn snapshot(&self) -> Box<dyn Any + Send + Sync>;

    fn flush(&mut self);
}

impl<T: Ealloc> AnyEalloc for T {
    fn as_any_mut(&mut self) -> &mut dyn Any { self }

    fn shards(&mut self, vec: &mut Vec<Box<dyn AnyShard>>) {
        Ealloc::shards(self, vec, |shard| Box::new(shard) as Box<dyn AnyShard>)
    }

    fn snapshot(&self) -> Box<dyn Any + Send + Sync> { Box::new(Ealloc::snapshot(self)) }

    fn flush(&mut self) { Ealloc::flush(self); }
}

// Default allocator

struct MutTakeIter<'t, T, I: Iterator<Item = T>>(&'t mut I, usize);

impl<'t, T, I: Iterator<Item = T>> Iterator for MutTakeIter<'t, T, I> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.1 == 0 {
            return None;
        }
        self.1 -= 1;
        self.0.next()
    }
}

fn iter_gaps<E: Raw>(
    gauge: E,
    breakpoints: impl Iterator<Item = E>,
) -> impl iter::FusedIterator<Item = ops::Range<E>> {
    enum Previous<E: Raw> {
        Initial,
        Breakpoint(E),
        Finalized,
    }
    struct IterGaps<E: Raw, I: Iterator> {
        gauge:       E,
        breakpoints: I,
        previous:    Previous<E>,
    }
    impl<E: Raw, I: Iterator<Item = E>> Iterator for IterGaps<E, I> {
        type Item = ops::Range<E>;

        fn next(&mut self) -> Option<ops::Range<E>> {
            let start = match self.previous {
                Previous::Initial => E::new().load_mut(),
                Previous::Breakpoint(previous) => previous.add(1),
                Previous::Finalized => return None,
            };
            let (previous, end) = match self.breakpoints.next() {
                None => (Previous::Finalized, self.gauge),
                Some(breakpoint) => (Previous::Breakpoint(breakpoint), breakpoint),
            };
            self.previous = previous;
            Some(start..end)
        }
    }
    impl<E: Raw, I: Iterator<Item = E>> iter::FusedIterator for IterGaps<E, I> {}

    IterGaps { gauge, breakpoints, previous: Previous::Initial }
        .filter(|range| range.start != range.end)
}

/// Contains entity allocators for all archetypes.
#[derive(Default)]
pub struct Map {
    pub(crate) map: HashMap<DbgTypeId, Box<dyn AnyEalloc>>,
}

impl Map {
    pub(crate) fn new(map: HashMap<DbgTypeId, Box<dyn AnyEalloc>>) -> Self { Self { map } }

    pub(crate) fn get<A: Archetype>(&mut self) -> &mut A::Ealloc {
        self.map
            .get_mut(&TypeId::of::<A>())
            .expect("Attempted to delete entity of unknown archetype")
            .as_any_mut()
            .downcast_mut()
            .expect("TypeId mismatch")
    }

    pub(crate) fn shards(&mut self, num_shards: usize) -> Vec<ShardMap> {
        let mut shard_maps: Vec<ShardMap> = (0..num_shards).map(|_| ShardMap::default()).collect();
        let mut shard_buf = Vec::with_capacity(num_shards);

        for (&ty, ealloc) in &mut self.map {
            ealloc.shards(&mut shard_buf);

            for (shard_id, shard) in shard_buf.drain(..).enumerate() {
                let map = shard_maps.get_mut(shard_id).expect("inconsistent num_shards");
                map.map.insert(
                    ty,
                    ShardMapEntry { snapshot: ealloc.snapshot(), cell: RefCell::new(shard) },
                );
            }
        }

        shard_maps
    }

    /// Creates a snapshot of the allocated entities for an archetype.
    ///
    /// For online access, get the snapshot through [`ShardMap::snapshot`] instead.
    /// This function is intended for offline access e.g. in unit tests.
    pub fn snapshot<A: Archetype>(&mut self) -> Snapshot<A::RawEntity> {
        Ealloc::snapshot(self.get::<A>())
    }
}

struct ShardMapEntry {
    snapshot: Box<dyn Any + Send + Sync>, // Snapshot<E>
    cell:     RefCell<Box<dyn AnyShard>>,
}

/// A map of shards assigned to a single worker thread.
#[derive(Default)]
pub struct ShardMap {
    // RefCell is `Send`; we just want interior mutability within the worker thread.
    map: HashMap<DbgTypeId, ShardMapEntry>,
}

/// Return value of [`ShardMap::borrow`].
pub type BorrowedShard<'t, A: Archetype> = impl ops::DerefMut<Target = impl Shard<Raw = A::RawEntity, Hint = <A::Ealloc as Ealloc>::AllocHint>>
    + 't;

impl ShardMap {
    /// Gets the mutable shard reference.
    pub fn get<A: Archetype>(
        &mut self,
    ) -> &mut impl Shard<Raw = A::RawEntity, Hint = <A::Ealloc as Ealloc>::AllocHint> {
        let shard = self.map.get_mut(&TypeId::of::<A>()).expect("Use of unregistered archetype");
        let shard: &mut <A::Ealloc as Ealloc>::Shard =
            shard.cell.get_mut().as_any_mut().downcast_mut().expect("TypeId mismatch");
        shard
    }

    /// Returns a snapshot that tells what entities were allocated during last offline.
    pub fn snapshot<A: Archetype>(&self) -> &Snapshot<A::RawEntity> {
        let shard = self.map.get(&TypeId::of::<A>()).expect("Use of unregistered archetype");
        shard.snapshot.downcast_ref().expect("TypeId mismatch")
    }

    /// Borrows the shard for an archetype through a [`RefCell`].
    pub fn borrow<A: Archetype>(&self) -> BorrowedShard<A> {
        let shard = self.map.get(&TypeId::of::<A>()).expect("Use of unregistered archetype");
        let shard = shard
            .cell
            .try_borrow_mut()
            .expect("The same system cannot have multiple `EntityCreator`s on the same archetype");
        cell::RefMut::map(shard, |shard| {
            let shard: &mut <A::Ealloc as Ealloc>::Shard =
                shard.as_any_mut().downcast_mut().expect("TypeId mismatch");
            shard
        })
    }
}

#[cfg(test)]
mod tests;
