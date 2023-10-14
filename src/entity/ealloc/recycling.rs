use std::collections::BTreeSet;
use std::sync::Arc;
use std::{iter, ops};

use parking_lot::Mutex;

use super::{iter_gaps, Ealloc, Shard, ShardAssigner, Snapshot};
use crate::entity::raw::Atomic;
use crate::entity::Raw;

mod recycler;
pub use recycler::{BTreeHint, Recycler};

type MutableShards<T> = Vec<Arc<Mutex<T>>>;

/// The default allocator supporting atomically-allocated new IDs and arbitrary recycler.
#[derive(Debug)]
pub struct Recycling<E: Raw, T: Recycler<E>, S: ShardAssigner> {
    /// Whether `mark_need_flush` was called.
    flush_mark:         bool,
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
            flush_mark: false,
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
    ) -> impl iter::FusedIterator<Item = ops::Range<E>> + '_ {
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
        self.flush_mark = false;

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

    fn mark_need_flush(&mut self) { self.flush_mark = true; }
    fn flush_if_marked(&mut self) {
        if self.flush_mark {
            self.flush();
        }
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

/// [`Shard`] implementation for [`Recycling`].
pub struct RecyclingShard<GaugeRef, RecyclerRef, ReuseQueueRef> {
    global_gauge: GaugeRef,
    recycler:     RecyclerRef,
    reuse_queue:  ReuseQueueRef,
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

#[cfg(test)]
mod tests;
