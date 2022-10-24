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

use crate::Archetype;

mod simple;
pub use simple::{Simple, SimpleInitStrategy, SimplePresence};

mod isotope;
pub use isotope::{Isotope, IsotopeInitStrategy};

pub mod discrim;
pub use discrim::Discrim;

pub(crate) mod any;
pub use any::{DepList, IsotopeInitFn, IsotopeIniter, Map, SimpleInitFn, SimpleIniter};

/// Marks that a component type is always present.
///
/// This trait must only be implemented by components that
/// implement [`Simple`] with [`Simple::PRESENCE`] set to [`SimplePresence::Required`].
///
/// Not implementing this trait does not result in any issues
/// except for ergonomic inconvenience when using getters on storages.
pub trait Must<A: Archetype> {}
