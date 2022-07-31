use crate::entity::ealloc::{self, Shard};
use crate::{comp, entity, world, Archetype};

/// An operation to be executed after join.
pub(crate) trait Operation: Send {
    fn run(
        self: Box<Self>,
        components: &mut world::Components,
        sync_globals: &mut world::SyncGlobals,
        ealloc_map: &mut ealloc::Map,
    );
}

/// Create an entity.
pub(crate) struct CreateEntity<A: Archetype> {
    /// The entity ID, which was already allocated.
    entity:     A::RawEntity,
    /// The component list.
    components: comp::Map<A>,
}

impl<A: Archetype> Operation for CreateEntity<A> {
    fn run(
        self: Box<Self>,
        components: &mut world::Components,
        sync_globals: &mut world::SyncGlobals,
        ealloc_map: &mut ealloc::Map,
    ) {
        world::init_entity(sync_globals, self.entity, components, self.components);
    }
}

/// A sharded store for offline operations.
pub(crate) struct Buffer {
    pub(crate) shards: Vec<BufferShard>,
}

impl Buffer {
    pub(crate) fn new(num_shards: usize) -> Self {
        let shards = (0..num_shards).map(|_| BufferShard::default()).collect();
        Self { shards }
    }

    pub(crate) fn drain(&mut self) -> impl Iterator<Item = Box<dyn Operation>> + '_ {
        self.shards.iter_mut().flat_map(|shard| shard.items.drain(..))
    }
}

/// A shard of offline operation store.
#[derive(Default)]
pub struct BufferShard {
    items: Vec<Box<dyn Operation>>,
}

impl BufferShard {
    pub fn create_entity_with_hint<A: Archetype>(
        &mut self,
        components: comp::Map<A>,
        ealloc_map: &mut ealloc::ShardMap,
        hint: <A::Ealloc as ealloc::Ealloc>::AllocHint,
    ) -> entity::Entity<A> {
        let ealloc_shard = ealloc_map.get::<A>();
        let entity = ealloc_shard.allocate(hint);

        self.items.push(Box::new(CreateEntity { entity, components }));

        entity::Entity::new_allocated(entity)
    }
}
