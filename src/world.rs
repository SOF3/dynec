//! The world stores the states of the game.

use std::any::{self};

use crate::util::DbgTypeId;
use crate::{comp, entity, Archetype, Entity};

mod builder;
pub use builder::Builder;

mod state;
pub use state::{Components, SendGlobals, UnsendGlobals};

pub(crate) mod storage;
pub(crate) mod typed;

mod scheduler;

/// A bundle encapsulates the systems and resources for a specific feature.
/// This can be used by library crates to expose their features as a single API.
pub trait Bundle {
    /// Schedules the systems used by this bundle.
    ///
    /// Scheduling a system automatically registers the archetypes and components used by the system.
    fn register(&self, builder: &mut Builder) {}

    /// Populates the world with entities and global states.
    fn populate(&self, world: &mut World) {}
}

/// Creates a dynec world from bundles.
pub fn new<'t>(bundles: impl IntoIterator<Item = &'t dyn Bundle> + Copy) -> World {
    new_with_concurrency(
        bundles,
        match std::thread::available_parallelism() {
            Ok(c) => c.get(),
            Err(err) => {
                log::error!("Cannot detect number of CPUs, parallelism disabled");
                0
            }
        },
    )
}

/// Creates a dynec world from bundles with threading disabled.
pub fn new_unthreaded<'t>(bundles: impl IntoIterator<Item = &'t dyn Bundle> + Copy) -> World {
    new_with_concurrency(bundles, 0)
}

/// Creates a dynec world from bundles and specify the number of worker threads
/// (not counting the main thread, which only executes thread-local tasks).
pub fn new_with_concurrency<'t>(
    bundles: impl IntoIterator<Item = &'t dyn Bundle> + Copy,
    concurrency: usize,
) -> World {
    let mut builder = Builder::new(concurrency);

    for bundle in bundles {
        bundle.register(&mut builder);
    }

    let mut world = builder.build();

    for bundle in bundles {
        bundle.populate(&mut world);
    }

    world
}

/// Identifies an archetype + component type.
pub(crate) struct ArchComp {
    arch: DbgTypeId,
    comp: DbgTypeId,
}

/// Describes a simple component type.
pub(crate) struct SimpleSpec {
    presence:     comp::SimplePresence,
    // TODO wrap SimpleInitStrategy in a trait object
    is_finalizer: bool,
}

impl SimpleSpec {
    fn of<A: Archetype, C: comp::Simple<A>>() -> Self {
        Self { presence: C::PRESENCE, is_finalizer: C::IS_FINALIZER }
    }
}

/// The data structure that stores all states in the game.
pub struct World {
    /// Stores the component states in a world.
    components:     Components,
    /// Stores the system-local states and the scheduler topology.
    scheduler:      scheduler::Scheduler,
    /// Global states that can be concurrently accessed by systems on other threads.
    send_globals:   SendGlobals,
    /// Global states that must be accessed on the main thread.
    unsend_globals: UnsendGlobals,
}

impl World {
    fn archetype<A: Archetype>(&self) -> &typed::Typed<A> {
        match self.components.archetypes.get(&DbgTypeId::of::<A>()) {
            Some(typed) => typed.as_any().downcast_ref().expect("TypeId mismatch"),
            None => panic!(
                "The archetype {} cannot be used because it is not used in any systems",
                any::type_name::<A>()
            ),
        }
    }

    fn archetype_mut<A: Archetype>(&mut self) -> &mut typed::Typed<A> {
        match self.components.archetypes.get_mut(&DbgTypeId::of::<A>()) {
            Some(typed) => typed.as_any_mut().downcast_mut().expect("TypeId mismatch"),
            None => panic!(
                "The archetype {} cannot be used because it is not used in any systems",
                any::type_name::<A>()
            ),
        }
    }

    pub fn execute(&mut self) {
        self.scheduler.execute_full_cycle(
            &self.components,
            &self.send_globals,
            &self.unsend_globals,
        );
    }

    /// Adds an entity to the world.
    pub fn create<A: Archetype>(&mut self, components: comp::Map<A>) -> Entity<A> {
        self.create_near::<entity::Entity<A>>(None, components)
    }

    /// Adds an entity to the world near another entity.
    pub fn create_near<E: entity::Ref>(
        &mut self,
        near: Option<E>,
        components: comp::Map<<E as entity::Ref>::Archetype>,
    ) -> Entity<<E as entity::Ref>::Archetype> {
        let typed = self.archetype_mut::<E::Archetype>();
        let id = typed.create_near(near.map(|raw| raw.id().0), components);
        Entity::new_allocated(id)
    }
}

#[cfg(test)]
mod tests;
