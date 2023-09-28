//! Component reading and writing.

use std::any::type_name;
use std::collections::HashMap;

use super::typed;
use crate::util::DbgTypeId;
use crate::Archetype;

pub(crate) mod isotope;
mod partition;
pub(crate) mod simple;
use partition::{mut_owned_par_iter_chunks_mut, mut_owned_par_iter_mut, PartitionAccessor};

/// Stores the component states in a world.
pub struct Components {
    pub(crate) archetypes: HashMap<DbgTypeId, Box<dyn typed::AnyTyped>>,
}

impl Components {
    /// Creates a dummy, empty component store used for testing.
    pub fn empty() -> Self { Self { archetypes: HashMap::new() } }

    /// Fetches the [`Typed`](typed::Typed) for the requested archetype.
    pub(crate) fn archetype<A: Archetype>(&self) -> &typed::Typed<A> {
        match self.archetypes.get(&DbgTypeId::of::<A>()) {
            Some(typed) => typed.as_any().downcast_ref().expect("TypeId mismatch"),
            None => panic!(
                "The archetype {} cannot be used because it is not used in any systems",
                type_name::<A>()
            ),
        }
    }

    /// Fetches the [`Typed`](typed::Typed) for the requested archetype.
    pub(crate) fn archetype_mut<A: Archetype>(&mut self) -> &mut typed::Typed<A> {
        match self.archetypes.get_mut(&DbgTypeId::of::<A>()) {
            Some(typed) => typed.as_any_mut().downcast_mut().expect("TypeId mismatch"),
            None => panic!(
                "The archetype {} cannot be used because it is not used in any systems",
                type_name::<A>()
            ),
        }
    }
}

#[cfg(test)]
#[allow(clippy::extra_unused_type_parameters)] // macro magic
mod _assert {
    static_assertions::assert_impl_all!(super::Components: Send, Sync);
}
