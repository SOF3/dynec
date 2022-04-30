//! The world stores the states of the game.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use crate::{component, entity, Archetype, Entity};

mod builder;
pub use builder::Builder;

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
    let mut builder = Builder::default();

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
    arch: TypeId,
    comp: TypeId,
}

/// Describes a simple component type.
pub(crate) struct SimpleSpec {
    presence:     component::SimplePresence,
    // TODO wrap SimpleInitStrategy in a trait object
    is_finalizer: bool,
}

impl SimpleSpec {
    fn of<A: Archetype, C: component::Simple<A>>() -> Self {
        Self { presence: C::PRESENCE, is_finalizer: C::IS_FINALIZER }
    }
}

/// The data structure that stores all states in the game.
pub struct World {
    archetypes: HashMap<TypeId, Box<dyn typed::AnyTyped>>,
}

impl World {
    /// Adds an entity to the world.
    pub fn create<A: Archetype>(&mut self, components: component::Map<A>) -> Entity<A> {
        self.create_near::<entity::Entity<A>>(None, components)
    }

    /// Adds an entity to the world near another entity.
    pub fn create_near<E: entity::Ref>(
        &mut self,
        near: Option<E>,
        mut components: component::Map<<E as entity::Ref>::Archetype>,
    ) -> Entity<<E as entity::Ref>::Archetype> {
        let typed = self
            .archetypes
            .get_mut(&TypeId::of::<E::Archetype>())
            .expect("Attempt to create entity of an archetype not used in any systems");
        let typed: &mut typed::Typed<E::Archetype> =
            typed.as_any_mut().downcast_mut().expect("Typed archetype mismatch");

        let id = match near {
            Some(hint) => typed.ealloc.allocate_near(hint.id().0),
            None => typed.ealloc.allocate(),
        };

        for (id, storage) in &mut typed.simple_storages {
            let storage = Arc::get_mut(storage).expect("storage arc was leaked");
            let storage = storage.get_mut();
            storage.init_extract_components(&mut components);
        }

        Entity::new_allocated(id)
    }
}

#[cfg(test)]
mod tests;
