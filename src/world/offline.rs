//! Operations queued to be executed after the cycle joins.

use crate::entity::{self, ealloc};
use crate::{comp, world, Archetype};

/// An operation to be executed after join.
pub(crate) trait Operation: Send {
    /// Performs the opreation during offline.
    fn run(
        self: Box<Self>,
        components: &mut world::Components,
        sync_globals: &mut world::SyncGlobals,
        unsync_globals: &mut world::UnsyncGlobals,
        ealloc_map: &mut ealloc::Map,
    ) -> OperationResult;
}

/// Result of an opreation.
pub(crate) enum OperationResult {
    /// The operation completed.
    Ok,
    /// The operation should be rerun after the next cycle.
    /// This should return self.
    QueueForRerun(Box<dyn Operation>),
}

/// Create an entity.
pub(crate) struct CreateEntity<A: Archetype> {
    /// The entity ID, which was already allocated.
    entity:   A::RawEntity,
    /// The entity ref count, only useful in debug mode.
    rc:       entity::MaybeArc,
    /// The component list.
    comp_map: comp::Map<A>,
}

impl<A: Archetype> Operation for CreateEntity<A> {
    fn run(
        self: Box<Self>,
        components: &mut world::Components,
        sync_globals: &mut world::SyncGlobals,
        _unsync_globals: &mut world::UnsyncGlobals,
        ealloc_map: &mut ealloc::Map,
    ) -> OperationResult {
        world::init_entity(sync_globals, self.entity, self.rc, components, self.comp_map);
        OperationResult::Ok
    }
}

pub(crate) struct DeleteEntity<A: Archetype> {
    entity: A::RawEntity,
}

impl<A: Archetype> Operation for DeleteEntity<A> {
    fn run(
        self: Box<Self>,
        components: &mut world::Components,
        sync_globals: &mut world::SyncGlobals,
        unsync_globals: &mut world::UnsyncGlobals,
        ealloc_map: &mut ealloc::Map,
    ) -> OperationResult {
        match world::flag_delete_entity::<A>(
            self.entity,
            components,
            sync_globals,
            unsync_globals,
            ealloc_map,
        ) {
            world::DeleteResult::Deleted => OperationResult::Ok,
            world::DeleteResult::Terminating => OperationResult::QueueForRerun(self),
        }
    }
}

/// A sharded store for offline operations.
pub(crate) struct Buffer {
    /// Queue of operations to rerun in the next drain cycle.
    rerun_queue:       Vec<Box<dyn Operation>>,
    /// Shards of queues for each worker thread.
    pub(crate) shards: Vec<BufferShard>,
}

impl Buffer {
    pub(crate) fn new(num_shards: usize) -> Self {
        let shards = (0..num_shards).map(|_| BufferShard::default()).collect();
        Self { rerun_queue: Vec::new(), shards }
    }

    pub(crate) fn drain_cycle(
        &mut self,
        mut run: impl FnMut(Box<dyn Operation>) -> OperationResult,
    ) {
        self.rerun_queue = self
            .rerun_queue
            .drain(..)
            .chain(self.shards.iter_mut().flat_map(|shard| shard.items.drain(..)))
            .filter_map(|op| match run(op) {
                OperationResult::Ok => None,
                OperationResult::QueueForRerun(op) => Some(op),
            })
            .collect();
    }
}

/// A shard of offline operation store.
#[derive(Default)]
pub struct BufferShard {
    items: Vec<Box<dyn Operation>>,
}

impl BufferShard {
    /// Creates an entity and queues for initialization.
    pub fn create_entity<A: Archetype>(
        &mut self,
        comp_map: comp::Map<A>,
        ealloc_map: &mut ealloc::ShardMap,
    ) -> entity::Entity<A> {
        self.create_entity_with_hint::<A>(comp_map, ealloc_map, Default::default())
    }

    /// Creates an entity and queues for initialization.
    pub fn create_entity_with_hint<A: Archetype>(
        &mut self,
        comp_map: comp::Map<A>,
        ealloc_map: &mut ealloc::ShardMap,
        hint: <A::Ealloc as entity::Ealloc>::AllocHint,
    ) -> entity::Entity<A> {
        self.create_entity_with_hint_and_shard(comp_map, ealloc_map.get::<A>(), hint)
    }

    /// Creates an entity and queues for initialization.
    pub fn create_entity_with_shard<
        A: Archetype,
        S: ealloc::Shard<Raw = A::RawEntity, Hint = <A::Ealloc as entity::Ealloc>::AllocHint> + ?Sized,
    >(
        &mut self,
        comp_map: comp::Map<A>,
        ealloc_shard: &mut S,
    ) -> entity::Entity<A> {
        self.create_entity_with_hint_and_shard(comp_map, ealloc_shard, Default::default())
    }

    /// Creates an entity and queues for initialization.
    pub fn create_entity_with_hint_and_shard<
        A: Archetype,
        S: ealloc::Shard<Raw = A::RawEntity, Hint = <A::Ealloc as entity::Ealloc>::AllocHint> + ?Sized,
    >(
        &mut self,
        comp_map: comp::Map<A>,
        ealloc_shard: &mut S,
        hint: <A::Ealloc as entity::Ealloc>::AllocHint,
    ) -> entity::Entity<A> {
        let entity = ealloc_shard.allocate(hint);

        let allocated = entity::Entity::new_allocated(entity);

        self.items.push(Box::new(CreateEntity { entity, comp_map, rc: allocated.rc.clone() }));

        allocated
    }

    /// Queues an entity deletion.
    pub fn delete_entity<A: Archetype, E: entity::Ref<Archetype = A>>(&mut self, entity: E) {
        let entity = entity.id();

        self.items.push(Box::new(DeleteEntity::<A> { entity }));
    }
}
