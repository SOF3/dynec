//! Manages entity ID allocation and deallocation.

use std::any::{Any, TypeId};
use std::cell::{self, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::{iter, ops};

use parking_lot::Mutex;
use rand::prelude::SliceRandom;
use rand::Rng;

use super::raw::Raw;
use crate::entity::raw::Atomic;
use crate::util::DbgTypeId;
use crate::Archetype;

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

/// A sharded entity ID allocator.
///
/// Each worker thread has mutable access to a shard in each cycle.
/// Between cycles, the shards are shuffled to new worker threads.
pub trait Shard: Send + 'static {
    /// The raw entity ID type.
    type Raw: Raw;

    /// The allocation hint for the underlying recycler.
    type Hint: Sized;

    /// Allocates an ID from the shard.
    fn allocate(&mut self, hint: Self::Hint) -> Self::Raw;
}

pub(crate) trait AnyShard: Send + 'static {
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn as_any_box(self: Box<Self>) -> Box<dyn Any>;
}

impl<T: Shard> AnyShard for T {
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn as_any_box(self: Box<Self>) -> Box<dyn Any> { self }
}

// Default allocator

type MutableShards<T> = Vec<Arc<Mutex<T>>>;

/// The default allocator supporting atomically-allocated new IDs and arbitrary recycler.
#[derive(Debug)]
pub struct Recycling<E: Raw, T: Recycler<E>, S: ShardAssigner> {
    /// The next ID to allocate into shards.
    global_gauge:       Arc<E::Atomic>,
    /// A sorted list of recycled IDs during the last join.
    recyclable:         Arc<BTreeSet<E>>,
    /// The actual IDs assigned to different shards.
    recycler_shards:    MutableShards<T>,
    /// The assigned shard.
    shard_assigner:     S,
    /// The queue of deallocated IDs to distribute.
    dealloc_queue:      Vec<E>,
    /// The queue of allocated IDs during online, to be synced to recyclable after join.
    reuse_queue_shards: MutableShards<Vec<E>>,
}

impl<E: Raw, T: Recycler<E>, S: ShardAssigner> Recycling<E, T, S> {
    /// Creates a new recycling allocator with a custom shard assigner.
    /// This can only be used for unit testing since the Archetype API does not support dynamic
    /// shard assigners.
    pub(crate) fn new_with_shard_assigner(num_shards: usize, shard_assigner: S) -> Self {
        let global_gauge = E::new();
        Self {
            global_gauge: Arc::new(global_gauge),
            recyclable: Arc::default(),
            recycler_shards: (0..num_shards).map(|_| Arc::default()).collect(),
            shard_assigner,
            dealloc_queue: Vec::new(),
            reuse_queue_shards: (0..num_shards).map(|_| Arc::default()).collect(),
        }
    }

    fn get_recycler_offline(sharded_recyclers: &mut MutableShards<T>, index: usize) -> &mut T {
        let arc = sharded_recyclers.get_mut(index).expect("index out of bounds");
        Arc::get_mut(arc).expect("shards are dropped in offline mode").get_mut()
    }

    fn get_reuse_queue_offline(
        reuse_queues: &mut MutableShards<Vec<E>>,
        index: usize,
    ) -> &mut Vec<E> {
        let arc = reuse_queues.get_mut(index).expect("index out of bounds");
        Arc::get_mut(arc).expect("shards are dropped in offline mode").get_mut()
    }

    fn iter_allocated_chunks_offline(
        &mut self,
    ) -> impl Iterator<Item = ops::Range<E>> + iter::FusedIterator + '_ {
        iter_gaps(self.global_gauge.load(), self.recyclable.iter().copied())
    }
}

impl<E: Raw, T: Recycler<E>, S: ShardAssigner> Ealloc for Recycling<E, T, S> {
    type Raw = E;
    type AllocHint = T::Hint;
    type Shard = impl Shard<Raw = E, Hint = T::Hint>;

    fn new(num_shards: usize) -> Self { Self::new_with_shard_assigner(num_shards, S::default()) }

    fn shards<U, F: Fn(Self::Shard) -> U>(&mut self, vec: &mut Vec<U>, f: F) {
        let slice_start = vec.len();

        vec.extend(
            iter::zip(self.recycler_shards.iter(), self.reuse_queue_shards.iter())
                .map(|(recycler, reuse_queue)| -> Self::Shard {
                    // The return type hint here is used to constrain the TAIT, don't delete it.
                    RecyclingShard {
                        global_gauge: Arc::clone(&self.global_gauge),
                        recycler:     Arc::clone(recycler).lock_arc(),
                        reuse_queue:  Arc::clone(reuse_queue).lock_arc(),
                    }
                })
                .map(f),
        );
        let my_slice = vec.get_mut(slice_start..).expect("just inserted");
        self.shard_assigner.shuffle_shards(my_slice);
    }

    fn snapshot(&self) -> Snapshot<Self::Raw> {
        Snapshot { gauge: self.global_gauge.load(), recyclable: Arc::clone(&self.recyclable) }
    }

    fn allocate(&mut self, hint: Self::AllocHint) -> Self::Raw {
        let shard_id =
            self.shard_assigner.select_for_offline_allocation(self.recycler_shards.len());
        let recycler = Self::get_recycler_offline(&mut self.recycler_shards, shard_id);
        let reuse_queue = Self::get_reuse_queue_offline(&mut self.reuse_queue_shards, shard_id);

        let mut shard = RecyclingShard { global_gauge: &*self.global_gauge, recycler, reuse_queue };
        shard.allocate(hint)
    }

    fn queue_deallocate(&mut self, id: E) { self.dealloc_queue.push(id); }

    fn flush(&mut self) {
        let mut ids = &self.dealloc_queue[..];
        {
            let recyclable = Arc::get_mut(&mut self.recyclable)
                .expect("all exposed shards should be dropped before flush");
            recyclable.extend(ids);
            for shard in &mut self.reuse_queue_shards {
                let queue = Arc::get_mut(shard)
                    .expect("all exposed shards should be dropped before flush")
                    .get_mut();

                for item in queue.drain(..) {
                    recyclable.remove(&item);
                }
            }
        }

        // try to distribute `ids` to all shards evenly.
        let mut shards: Vec<_> = self
            .recycler_shards
            .iter_mut()
            .map(|recycler| {
                Arc::get_mut(recycler)
                    .expect("all exposed shards should be dropped before flush")
                    .get_mut()
            })
            .collect();

        // the shards with the smallest recycle count come first because we assign IDs to them
        // first.
        shards.sort_by_key(|recycler| recycler.len());

        let mut target_sizes: Vec<_> = shards.iter().map(|recycler| recycler.len()).collect();
        distribute_sorted(&mut target_sizes, ids.len());

        for (i, recycler) in shards.iter_mut().enumerate() {
            let take: usize =
                *target_sizes.get(i).expect("target_sizes collected from shards.iter()")
                    - recycler.len();
            // take >= 0 assuming correctness of distribute_sorted

            let (left, right) = ids.split_at(take);
            recycler.extend(left.iter().copied());
            ids = right;
        }

        self.dealloc_queue.clear();
    }
}

fn distribute_sorted(sizes: &mut [usize], total: usize) {
    let mut added = 0;
    let mut target = 0;

    let mut shards_used = 0;
    for (i, &size) in sizes.iter().enumerate() {
        let delta = (size - target) * i;
        if added + delta >= total {
            break;
        }

        added += delta;
        target = size;
        shards_used += 1;
    }
    if shards_used == 0 {
        return; // no shards need updating
    }

    let deficit = total - added;
    target += deficit / shards_used;
    let remainder = deficit % shards_used;

    let (left, right) = sizes[..shards_used].split_at_mut(shards_used - remainder);
    left.fill(target);
    right.fill(target + 1);
}

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

/// [`Shard`] implementation for [`Recycling`].
pub struct RecyclingShard<GaugeRef, RecyclerRef, ReuseQueueRef> {
    global_gauge: GaugeRef,
    recycler:     RecyclerRef,
    reuse_queue:  ReuseQueueRef,
}

impl<E: Raw, T: Recycler<E>, GaugeRef, RecyclerRef, ReuseQueueRef>
    RecyclingShard<GaugeRef, RecyclerRef, ReuseQueueRef>
where
    GaugeRef: ops::Deref<Target = E::Atomic>,
    RecyclerRef: ops::DerefMut<Target = T>,
    ReuseQueueRef: ops::DerefMut<Target = Vec<E>>,
{
    fn allocate(&mut self, hint: T::Hint) -> E {
        if let Some(id) = self.recycler.poll(hint) {
            self.reuse_queue.push(id);
            id
        } else {
            self.global_gauge.fetch_add(1)
        }
    }
}

fn iter_gaps<E: Raw>(
    gauge: E,
    breakpoints: impl Iterator<Item = E>,
) -> impl Iterator<Item = ops::Range<E>> + iter::FusedIterator {
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

impl<E: Raw, GaugeRef, RecyclerRef, ReuseQueueRef> Shard
    for RecyclingShard<GaugeRef, RecyclerRef, ReuseQueueRef>
where
    GaugeRef: ops::Deref<Target = E::Atomic> + Send + 'static,
    RecyclerRef: ops::DerefMut + Send + 'static,
    <RecyclerRef as ops::Deref>::Target: Recycler<E>,
    ReuseQueueRef: ops::DerefMut<Target = Vec<E>> + Send + 'static,
{
    type Raw = E;
    type Hint = <RecyclerRef::Target as Recycler<E>>::Hint;

    fn allocate(&mut self, hint: Self::Hint) -> E {
        if let Some(id) = self.recycler.poll(hint) {
            id
        } else {
            self.global_gauge.fetch_add(1)
        }
    }
}

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

/// Provides the randomness for shard assignment.
pub trait ShardAssigner: Default + 'static {
    /// Selects a shard for offline allocation.
    fn select_for_offline_allocation(&mut self, num_shards: usize) -> usize;

    /// Shuffles shards for worker thread dispatch.
    fn shuffle_shards<T>(&mut self, shards: &mut [T]);
}

/// The default shard assigner using [`rand::thread_rng`].
#[derive(Default)]
pub struct ThreadRngShardAssigner;

impl ShardAssigner for ThreadRngShardAssigner {
    fn select_for_offline_allocation(&mut self, num_shards: usize) -> usize {
        rand::thread_rng().gen_range(0..num_shards)
    }

    fn shuffle_shards<T>(&mut self, shards: &mut [T]) { shards.shuffle(&mut rand::thread_rng()); }
}

/// A shard assigner that never shuffles and always allocates from the same shard.
/// Used for testing only.
#[derive(Debug, Default)]
pub struct StaticShardAssigner {
    /// The shard always returned for [`ShardAssigner::select_for_offline_allocation`]
    pub allocating_shard: usize,
}

impl ShardAssigner for StaticShardAssigner {
    fn select_for_offline_allocation(&mut self, _num_shards: usize) -> usize {
        self.allocating_shard
    }

    fn shuffle_shards<T>(&mut self, _shards: &mut [T]) {
        // no shuffling
    }
}

#[derive(Default)]
pub(crate) struct Map {
    pub(crate) map: HashMap<DbgTypeId, Box<dyn AnyEalloc>>,
}

impl Map {
    pub(crate) fn new(map: HashMap<DbgTypeId, Box<dyn AnyEalloc>>) -> Self { Self { map } }

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
}

// TODO change this into a trait to allow non-recycling ealloc
/// A snapshot of the allocated entities during offline.
#[derive(Clone)]
pub struct Snapshot<E> {
    gauge:      E,
    recyclable: Arc<BTreeSet<E>>,
}

impl<E: Raw> Snapshot<E> {
    /// Iterates over all chunks of allocated entities.
    pub fn iter_allocated_chunks(
        &self,
    ) -> impl Iterator<Item = ops::Range<E>> + iter::FusedIterator + '_ {
        iter_gaps(self.gauge, self.recyclable.iter().copied())
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
    pub fn borrow<A: Archetype>(
        &self,
    ) -> impl ops::DerefMut<
        Target = impl Shard<Raw = A::RawEntity, Hint = <A::Ealloc as Ealloc>::AllocHint>,
    > + '_ {
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
