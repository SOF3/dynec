use super::{SimpleInitFn, SimpleIniter};
use crate::{entity, world, Archetype};

/// A simple component has only one instance per entity.
///
/// See the [module-level documentation](mod@crate::comp) for more information.
pub trait Simple<A: Archetype>: entity::Referrer + Send + Sync + Sized + 'static {
    /// The presence constraint of this component.
    const PRESENCE: SimplePresence;

    /// The initialization strategy for this component.
    const INIT_STRATEGY: SimpleInitStrategy<A>;

    /// Override this to `true` if the component is a finalizer.
    ///
    /// Finalizer components must be [optional](SimplePresence::Optional).
    /// Entities are not removed until all finalizer components have been removed.
    const IS_FINALIZER: bool = false;

    /// The storage type used for storing this simple component.
    type Storage: world::Storage<RawEntity = A::RawEntity, Comp = Self>;
}

/// Describes whether a simple component must be present.
pub enum SimplePresence {
    /// The component may not be present in an entity.
    /// The component is always retrieved as an `Option` type.
    Optional,

    /// The component must be present in an entity.
    /// It can be mutated, but it cannot be removed from the entity.
    ///
    /// If it is not given in the entity creation args
    /// and its [`SimpleInitStrategy`] is not [`Auto`](SimpleInitStrategy::Auto),
    /// entity creation will panic.
    Required,
}

/// Describes how a simple component is auto-initialized.
pub enum SimpleInitStrategy<A: Archetype> {
    /// The component is not auto-initialized.
    None,
    /// The component should be auto-initialized using the [`any::AutoIniter`]
    /// if it is not given in the creation args.
    Auto(SimpleIniter<A>),
}

impl<A: Archetype> SimpleInitStrategy<A> {
    /// Constructs an auto-initializing init strategy from a closure.
    pub fn auto(f: &'static impl SimpleInitFn<A>) -> Self { Self::Auto(SimpleIniter { f }) }
}
