//! An archetype is a kind of entity with a fixed set of (optional) components.
//!
//! It is comparable to a class in OOP-based designs.
//!
//! # Declaration
//! Use the [`crate::archetype!`] macro to declare an archetype.
//!
//! # Definition
//! The actual component types of an archetype are specified by
//! making the component type implement [`crate::comp::Simple<A>`] or [`crate::comp::Isotope<A>`],
//! where `A` is the archetype type.
//! Since Rust allows externally declared types to implement traits
//! if the trait has a type parameter declared in the current crate,
//! this means you can add components to archetypes declared in a dependency crate.
//!
//! # Registration
//! Archetypes and components are registered to the world
//! when a system that uses this archetype-component pair is scheduled.

/// An archetype is a kind of entity with a fixed set of (optional) components.
///
/// See the [module-level documentation](mod@crate::archetype) for more information.
pub trait Archetype: Send + Sync + 'static {}
