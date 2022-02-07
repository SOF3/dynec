//! Archetypes identify the components that an entity should contain.

use crate::Component;

/// An archetype is a type of entities.
///
/// Archetypes are never constructed. Implementors should be empty enums.
pub trait Archetype: 'static {}

/// If an archetype `A` can contain component `C`,
/// then `A: archetype::Contains<C>`.
pub trait Contains<C: Component>: Archetype {}
