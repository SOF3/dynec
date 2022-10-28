//! Manages entity ID allocation and deallocation.

use std::any::{Any, TypeId};
use std::cell::{self, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::marker::PhantomData;
use std::ops;
use std::sync::Arc;

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

    /// Returns a vector of shards where each shard references internal states in the allocator.
    ///
    /// The length of the result must be `num_shards` in [`Ealloc::new`].
    /// The caller shall shuffle the results returned by this method.
    fn shards<U, F: Fn(Self::Shard) -> U>(&mut self, vec: &mut Vec<U>, f: F);

    /// Allocate an ID.
    /// Can only be used between out-of-cycle.
    fn allocate(&mut self, hint: Self::AllocHint) -> Self::Raw;

    /// Queues the deallocation of an ID.
    fn queue_deallocate(&mut self, id: Self::Raw);

    /// Flushes the ID deallocation queue.
    fn flush_deallocate(&mut self);
}

pub(crate) trait AnyEalloc {
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn shards(&mut self, vec: &mut Vec<Box<dyn AnyShard>>);

    fn flush_deallocate(&mut self);
}

impl<T: Ealloc> AnyEalloc for T {
    fn as_any_mut(&mut self) -> &mut dyn Any { self }

    fn shards(&mut self, vec: &mut Vec<Box<dyn AnyShard>>) {
        Ealloc::shards(self, vec, |shard| Box::new(shard) as Box<dyn AnyShard>)
    }

    fn flush_deallocate(&mut self) { Ealloc::flush_deallocate(self); }
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

/// The default allocator supporting atomically-allocated new IDs and arbitrary recycler.
pub struct Recycling<R: Raw, T: Recycler<R>, S: ShardAssigner> {
    /// The next ID to allocate into shards.
    global_gauge:   Arc<R::Atomic>,
    /// The set of freed entity IDs.
    shards:         Vec<Arc<Mutex<RecyclingShardState<R, T>>>>,
    /// The assigned shard.
    shard_assigner: S,
    /// The queue of deallocated IDs to distribute.
    dealloc_queue:  Vec<R>,
}

impl<R: Raw, T: Recycler<R>, S: ShardAssigner> Recycling<R, T, S> {
    /// Creates a new recycling allocator with a custom shard assigner.
    /// This can only be used for unit testing since the Archetype API does not support dynamic
    /// shard assigners.
    pub(crate) fn new_with_shard_assigner(num_shards: usize, shard_assigner: S) -> Self {
        let global_gauge = R::new();
        let shards = (0..num_shards)
            .map(|_| {
                Arc::new(Mutex::new(RecyclingShardState {
                    recycler: T::default(),
                    _ph:      PhantomData,
                }))
            })
            .collect();
        Self {
            global_gauge: Arc::new(global_gauge),
            shard_assigner,
            shards,
            dealloc_queue: Vec::new(),
        }
    }

    /// Gets a shard state in offline mode.
    #[cfg(test)]
    fn offline_shard(&mut self, i: usize) -> &mut RecyclingShardState<R, T> {
        Arc::get_mut(self.shards.get_mut(i).expect("Undefined shard index"))
            .expect("Offline Arc leak")
            .get_mut()
    }
}

impl<R: Raw, T: Recycler<R>, S: ShardAssigner> Ealloc for Recycling<R, T, S> {
    type Raw = R;
    type AllocHint = T::Hint;
    type Shard = RecyclingShard<R, T>;

    fn new(num_shards: usize) -> Self { Self::new_with_shard_assigner(num_shards, S::default()) }

    fn shards<U, F: Fn(Self::Shard) -> U>(&mut self, vec: &mut Vec<U>, f: F) {
        // TODO optimization: see if we can optimize away the arc cloning cost
        // by reusing the same shard instances in every loop
        let slice_start = vec.len();
        vec.extend(
            self.shards
                .iter_mut()
                .map(|state| RecyclingShard {
                    global_gauge: Arc::clone(&self.global_gauge),
                    state:        Arc::clone(state),
                })
                .map(f),
        );
        let my_slice = vec.get_mut(slice_start..).expect("just inserted");
        self.shard_assigner.shuffle_shards(my_slice);
    }

    fn allocate(&mut self, hint: Self::AllocHint) -> Self::Raw {
        // TODO optimization: get rid of the useless Arc clone
        let shard_id = self.shard_assigner.select_for_offline_allocation(self.shards.len());
        let shard =
            self.shards.get_mut(shard_id).expect("shard_id was selected from 0..self.shards.len()");
        let mut shard = RecyclingShard::<R, T> {
            global_gauge: Arc::clone(&self.global_gauge),
            state:        Arc::clone(shard),
        };
        shard.allocate(hint)
    }

    fn queue_deallocate(&mut self, id: R) { self.dealloc_queue.push(id); }

    fn flush_deallocate(&mut self) {
        let mut shards: Vec<&mut RecyclingShardState<R, T>> = self
            .shards
            .iter_mut()
            .map(|shard| Arc::get_mut(shard).expect("Leaked Arc").get_mut())
            .collect();

        let mut ids = &self.dealloc_queue[..];

        // try to distribute `ids` to all shards evenly.

        // the shards with the smallest recycle count come first because we assign IDs to them
        // first.
        shards.sort_by_key(|state| state.recycler.len());

        let mut target_sizes: Vec<_> = shards.iter().map(|shard| shard.recycler.len()).collect();
        distribute_sorted(&mut target_sizes, ids.len());

        for (i, shard) in shards.iter_mut().enumerate() {
            let take: usize =
                *target_sizes.get(i).expect("target_sizes collected from shards.iter()")
                    - shard.recycler.len();
            // take >= 0 assuming correctness of distribute_sorted

            let (left, right) = ids.split_at(take);
            shard.recycler.extend(left.iter().copied());
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

struct RecyclingShardState<R: Raw, T: Recycler<R>> {
    recycler: T,
    _ph:      PhantomData<R>,
}

struct JoinState<R: Raw> {
    recycle_list: Vec<R>,
}

/// [`Shard`] implementation for [`Recycling`].
pub struct RecyclingShard<R: Raw, T: Recycler<R>> {
    global_gauge: Arc<R::Atomic>,
    state:        Arc<Mutex<RecyclingShardState<R, T>>>,
}

impl<R: Raw, T: Recycler<R>> Shard for RecyclingShard<R, T> {
    type Raw = R;
    type Hint = T::Hint;

    fn allocate(&mut self, hint: T::Hint) -> R {
        let mut state = self.state.try_lock().expect("Lock contention"); // TODO optimize this, don't request lock every time

        if let Some(id) = state.recycler.poll(hint) {
            id
        } else {
            self.global_gauge.fetch_add(1)
        }
    }
}

/// A data structure that provides the ability to recycle entity IDs.
pub trait Recycler<R: Raw>: Default + Extend<R> + Send + 'static {
    /// Additional configuration for polling.
    type Hint: Default;

    /// Returns the length of this recycler.
    fn len(&self) -> usize;

    /// Returns whether the recycler is empty.
    fn is_empty(&self) -> bool { self.len() == 0 }

    /// Polls an ID from the recycler based on the given hint.
    fn poll(&mut self, hint: Self::Hint) -> Option<R>;
}

/// A minimal allocator implemented through a FILO stack.
impl<R: Raw> Recycler<R> for Vec<R> {
    type Hint = ();

    fn len(&self) -> usize { Vec::len(self) }

    fn poll(&mut self, (): ()) -> Option<R> { self.pop() }
}

/// Additional configuration for allocating entities from a BTreeSet recycler.
pub struct BTreeHint<R> {
    /// Try to allocate the entity somewhere nearest to the given value.
    pub near: Option<R>,
}

impl<R: Raw> Default for BTreeHint<R> {
    fn default() -> Self { Self { near: None } }
}

impl<R: Raw> Recycler<R> for BTreeSet<R> {
    type Hint = BTreeHint<R>;

    fn len(&self) -> usize { BTreeSet::len(self) }

    fn poll(&mut self, hint: Self::Hint) -> Option<R> {
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
#[derive(Default)]
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
                map.map.insert(ty, RefCell::new(shard));
            }
        }

        shard_maps
    }
}

/// A map of shards assigned to a single worker thread.
#[derive(Default)]
pub struct ShardMap {
    map: HashMap<DbgTypeId, RefCell<Box<dyn AnyShard>>>,
}

impl ShardMap {
    /// Gets the mutable shard reference.
    pub fn get<A: Archetype>(
        &mut self,
    ) -> &mut impl Shard<Raw = A::RawEntity, Hint = <A::Ealloc as Ealloc>::AllocHint> {
        let shard = self.map.get_mut(&TypeId::of::<A>()).expect("Use of unregistered archetype");
        let shard: &mut <A::Ealloc as Ealloc>::Shard =
            shard.get_mut().as_any_mut().downcast_mut().expect("TypeId mismatch");
        shard
    }

    /// Borrows the shard for an archetype through a [`RefCell`].
    pub fn borrow<A: Archetype>(
        &self,
    ) -> impl ops::DerefMut<
        Target = impl Shard<Raw = A::RawEntity, Hint = <A::Ealloc as Ealloc>::AllocHint>,
    > + '_ {
        let shard = self.map.get(&TypeId::of::<A>()).expect("Use of unregistered archetype");
        let shard = shard.borrow_mut();
        cell::RefMut::map(shard, |shard| {
            let shard: &mut <A::Ealloc as Ealloc>::Shard =
                shard.as_any_mut().downcast_mut().expect("TypeId mismatch");
            shard
        })
    }
}

#[cfg(test)]
mod tests;
