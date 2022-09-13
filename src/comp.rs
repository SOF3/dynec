//! A component is a small data structure that can be attached to an entity.
//!
//! dynec is a statically-archetyped ECS framework.
//! Components can only be attached to entities of an archetype `A`
//! where the component type implements `Simple<A>` or `Isotope<A>`.
//!
//! # Simple vs Isotope components
//! A component type must implement either [`Simple`] or [`Isotope`] to be useful.
//!
//! Component types that implement [`Simple`] are called simple components.
//! There can only be one instance of each simple component type for each entity.
//! This means that each entity is effectively a typemap of its simple components.
//!
//! Component types that implement [`Isotope`] are called isotope components.
//! For an isotope component type `C`,
//! there can be multiple instances of `C` stored on the same entity,
//! indexed by [its discriminant type](Isotope::Discrim) [`Discrim`].
//! You can consider isotope components to be basically simple components with the type `Vec<C>`,
//! except with [more efficient storage](#storage).
//! Note that dynec instantiates a new storage for each discriminant,
//! so there should be a reasonably small number of distinct discriminants.
//!
//! # Registration
//! Archetypes and components are registered to the world
//! when a system that uses this archetype-component pair is scheduled.
//!
//! # Storage
//! For simple components, components of the same component type and archetype are stored together.
//!
//! For isotope components, components of the same component type, same archetype *and same
//! discriminant* are stored together.
//! This means each (component type + discriminant) combination is considered as a different
//! component type.
//!
//! For each storage, the components are either stored in a `Vec<MaybeUninit<C>>` or a `BTreeMap<C>`.
//!
//! If a simple component is optional and the storage is a `Vec<MaybeUninit<C>>`,
//! its presence in each entity is stored in a bitvec.
//! Isotope components are either lazy-initialized or optional,
//! so they always use a bitvec to store presence.
//!
//! # Instantiation
//! When an entity is created, its simple components are auto-instantiated based on the [`SimpleInitStrategy`]
//! specified in [`Simple::INIT_STRATEGY`] if it is absent in the creation args.
//!
//! Isotope components are never instantiated on entity creation.

use std::any::type_name;

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

/// Describes whether a component must be present.
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

/// Describes how a component is auto-initialized.
pub enum SimpleInitStrategy<A: Archetype> {
    /// The component is not auto-initialized.
    None,
    /// The component should be auto-initialized using the [`any::AutoIniter`]
    /// if it is not given in the creation args.
    Auto(any::AutoIniter<A>),
}

impl<A: Archetype> SimpleInitStrategy<A> {
    /// Constructs an auto-initializing init strategy from a closure.
    pub fn auto(f: &'static impl any::AutoInitFn<A>) -> Self { Self::Auto(any::AutoIniter { f }) }
}

/// An isotope component may have multiple instances per entity.
///
/// See the [module-level documentation](mod@crate::comp) for more information.
pub trait Isotope<A: Archetype>: entity::Referrer + Send + Sync + Sized + 'static {
    /// The initialization strategy for this component.
    const INIT_STRATEGY: IsotopeInitStrategy<Self>;

    /// The discriminant type.
    type Discrim: Discrim;

    /// The storage type used for storing this simple component.
    type Storage: world::Storage<RawEntity = A::RawEntity, Comp = Self>;
}

/// Describes how an isotope component is auto-initialized.
pub enum IsotopeInitStrategy<T> {
    /// The component is not auto-initialized.
    /// The component is always retrieved as an `Option` type.
    None,
    /// The component should be auto-initialized using the given function
    /// if it is not already present when retrieved.
    ///
    /// For immutable access, if the value is not already present,
    /// the function is invoked each time the component is requested
    /// to pass the result to the system,
    /// but the result is not stored to avoid acquiring mutable access to the storage.
    /// Therefore, the function should be cheap, e.g. just creating a zero value.
    ///
    /// For mutable access, if the value is not already present,
    /// the function is invoked and the result is stored in the storage,
    /// then the system is given a mutable reference to the value in the storage.
    Default(fn() -> T),
}

pub(crate) fn must_isotope_init<A: Archetype, C: Isotope<A> + Must<A>>() -> C {
    match C::INIT_STRATEGY {
        IsotopeInitStrategy::None => {
            panic!(
                "{}: comp::Must<{}> but it has no init strategy for this archetype",
                type_name::<C>(),
                type_name::<A>(),
            )
        }
        IsotopeInitStrategy::Default(fp) => fp(),
    }
}

/// Marks that a component type is always present.
///
/// This trait must only be implemented by components that
/// either implement [`Simple`] with [`Simple::PRESENCE`] set to [`SimplePresence::Required`]
/// or implement [`Isotope`] with [`Isotope::INIT_STRATEGY`] set to [`IsotopeInitStrategy::Default`].
///
/// Not implementing this trait does not result in any issues
/// except for ergonomic inconvenience when using getters on storages.
pub trait Must<A: Archetype> {}

pub(crate) mod any;
pub use any::{AutoInitFn, AutoIniter, Map};

pub mod discrim;
pub use discrim::Discrim;
