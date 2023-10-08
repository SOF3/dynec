use super::TestArch;
use crate::{entity, global, Entity};

/// A generic global state with an initializer.
#[global(dynec_as(crate), initial)]
#[derive(Default)]
pub struct Aggregator {
    pub comp30_sum:     i32,
    pub comp41_product: i32,
}

/// An entity-referencing global state.
#[global(dynec_as(crate), initial)]
#[derive(Default)]
pub struct InitialEntities {
    /// A strong reference.
    #[entity]
    pub strong: Option<Entity<TestArch>>,
    /// A weak reference.
    #[entity]
    pub weak:   Option<entity::Weak<TestArch>>,
}
