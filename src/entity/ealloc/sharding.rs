use std::any::Any;

use rand::seq::SliceRandom as _;
use rand::Rng as _;

use crate::entity::Raw;

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
