//! The world stores the states of the game.

use std::any::{self, TypeId};
use std::collections::HashMap;

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
    fn archetype<A: Archetype>(&self) -> &typed::Typed<A> {
        match self.archetypes.get(&TypeId::of::<A>()) {
            Some(typed) => typed.as_any().downcast_ref().expect("TypeId mismatch"),
            None => panic!(
                "The archetype {} cannot be used because it is not used in any systems",
                any::type_name::<A>()
            ),
        }
    }

    fn archetype_mut<A: Archetype>(&mut self) -> &mut typed::Typed<A> {
        match self.archetypes.get_mut(&TypeId::of::<A>()) {
            Some(typed) => typed.as_any_mut().downcast_mut().expect("TypeId mismatch"),
            None => panic!(
                "The archetype {} cannot be used because it is not used in any systems",
                any::type_name::<A>()
            ),
        }
    }

    /// Adds an entity to the world.
    pub fn create<A: Archetype>(&mut self, components: component::Map<A>) -> Entity<A> {
        self.create_near::<entity::Entity<A>>(None, components)
    }

    /// Adds an entity to the world near another entity.
    pub fn create_near<E: entity::Ref>(
        &mut self,
        near: Option<E>,
        components: component::Map<<E as entity::Ref>::Archetype>,
    ) -> Entity<<E as entity::Ref>::Archetype> {
        let typed = self.archetype_mut::<E::Archetype>();
        let id = typed.create_near(near.map(|raw| raw.id().0), components);
        Entity::new_allocated(id)
    }
}

#[cfg(test)]
mod tests;
