//! An opinionated ECS-like framework.
//!
//! - Entities are explicitly archetyped.
//!   All entities with the same archetype must have the same set of component types.
//! - Each entity can have up to `n` components of the same type for a small `n`.
//!   `BTreeMap` is used for components with small density
//!   and `Vec` for components with high density.
//! - Entities are reference-counted when `debug_assertions` is on.
//!   The game panics if entities are deleted when there are still dangling references.
//! - Entity order can be rearranged between simulation cycles.
//!   This allows better cache locality by arranging frequently accessed entities together,
//!   e.g. by sorting entities in an octree.

#![deny(
    anonymous_parameters,
    bare_trait_objects,
    clippy::clone_on_ref_ptr,
    clippy::float_cmp_const,
    clippy::if_not_else,
    clippy::unwrap_used
)]
#![cfg_attr(
    debug_assertions,
    allow(
        dead_code,
        unused_imports,
        unused_variables,
        clippy::match_single_binding,
    )
)]
#![cfg_attr(any(doc, not(debug_assertions)), deny(missing_docs))]
#![cfg_attr(
    not(debug_assertions),
    deny(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::dbg_macro,
    )
)]

#[doc(inline)]
pub use dynec_codegen::{system, Archetype, Component};

pub mod archetype;
#[doc(inline)]
pub use archetype::Archetype;

pub mod component;
#[doc(inline)]
pub use component::Component;

mod entity;
pub use entity::Entity;

pub mod system;
pub use system::System;

mod storage;
pub mod world;
#[doc(inline)]
pub use world::World;

mod optvec;
mod syncmap;

/// A global resource stored independently of entities and archetypes.
pub trait Global: Sized {}
