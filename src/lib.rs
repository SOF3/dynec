//! An opinionated ECS-like framework.
//!
//! [![CI](https://github.com/SOF3/dynec/actions/workflows/ci.yml/badge.svg)](https://github.com/SOF3/dynec/actions/workflows/ci.yml)
//! [![codecov](https://codecov.io/gh/SOF3/dynec/branch/master/graph/badge.svg?token=WAU2FOLHVW)](https://codecov.io/gh/SOF3/dynec)
//!
//! # What is ECS?
//! ECS is a data-oriented programming paradigm that focuses on optimizing the CPU cache.
//! Objects ("Entities") store their data in "Components",
//! which are processed in "Systems".
//!
//! # dynec has E, C and S, but it is not the typical ECS
//! dynec is **statically archetyped**.
//! The archetype of an entity refers to the set of components it can have,
//! which is comparable to the *class* of an object in OOP.
//! In dynec, entities cannot change to another archetype once created.
//! Entities can still have optional components, but they must be known in advance.
//!
//! This allows entity references to be strictly typed.
//! When you hold an entity reference,
//! you are assured that all entities in the reference are present.
//! Entities of different archetypes are stored separately,
//! which further improves *cache locality*
//! (since components of different archetypes are mostly unrelated).
//! Components can also declare that they must always be present on entities of an archetype,
//! which give you more confidence that the component really exists.
//!
//! Furthermore, archetypes cannot be subtyped.
//! This means that, unlike the traditional ECS,
//! there is no "join query" that queries all entities with a subset of components present.
//! Iteration can only be performed on all entities of an archetype
//! (it is also possible to iterate over all entities with a single component,
//! but this is for a different purpose).
//! If you want to fetch all entities with all of multiple components
//! like how you would usually do in other frameworks,
//! you probably wanted to split them to be a separate entity instead.
//!
//! ## Doesn't this make polymorphism more difficult to use?
//! I imagine your design is like this:
//! some entities have the "pig" archetype,
//! some have the "bird" archetype,
//! both share the common animal components,
//! while "bird" also has the additional flight- and egg-related components.
//!
//! This is not the perspective to organize entities in dynec.
//! Pigs and birds are both the same archetype, let's say "animal".
//! The term "bird" is just an umbrella term to refer to abilities such as flying and laying eggs.
//! It should not be a concept that appears in the code logic at all,
//! because "bird" does not really mean anything at the programming level.
//! In a sense, entities in dynec are comparable to some optional components in traditional ECS.
//!
//! This implies entities would have a lot of references among them &mdash;
//! a bird entity needs to reference its flight management entity and egg-laying entity.
//! Although this seems to complicate the design a lot,
//! this is actually inevitable in high-quality software
//! where one-to-one relationship is probably rare compared to one-to-many relationship.
//! For example, a bird may lay multiple types of eggs,
//! which would result in multiple egg-laying entities;
//! this cannot be trivially managed anyway.
//!
//! # Entities are (optionally) reference-counted and trackable
//! When debug assertions are enabled, all entity references are counted.
//! When an entity is deleted, dynec panics if there are still dangling references to the entity
//! and searches for the dangling references from all components and states in the world.
//! This means we can be (mostly) confident that any entity reference points to a live one,
//! and enables reduction of the size of a strong entity reference to one integer
//! because strong reference should not be able to outlive the referenced entity
//! (most ECS frameworks require another integer to store the "generation"
//! to avoid dangling references from pointing to a new entity recreated at the same offset).
//!
//! Dropping all references before an entity is deleted sounds troublesome to implement,
//! but dynec provides two solutions for this.
//! First, dynec supports "finalizer components",
//! where components serve as [asynchronous finalizers][k8s-finalizer].
//! Systems can create finalizers that ensure that references to the entity are dropped
//! before the actual deletion (and the dangling reference check) takes place.
//! This gives different systems the chance to clean up an entity
//! without losing the context that describes the entity
//! (because components are dropped after deletion and cannot be read anymore).
//! Second, if it is really necessary to retain the (dangling) entity reference,
//! you can store a "weak reference" instead &mdash;
//! weak references are also reference-counted, but they do not cause a dangling reference panic.
//!
//! Nevertheless, in order to track where entities are located,
//! all components and global and system-local states (basically all storages managed by dynec)
//! must implement a trait that supports scanning all owned strong/weak entity references.
//! dynec provides a derive macro to achieve this,
//! but since Rust does not support specialization (yet),
//! an `#[entity]` attribute needs to be applied on every field that may reference entities.
//! However, with the use of static assertions,
//! most mistakes in implementing the trait can be revealed at compile time
//! (exceptions are types with generic parameters, which require manual confirmation).
//! Despite all the trouble, the ability to scan for entity references make more features feasible,
//! including automatic system dependency declaration and entity rearrangement (described below).
//!
//! # Entities can have multiple components of the same type
//! How would we store the health of a player?
//! We create a `Health` component for player entities.
//! What if we also want to store the hunger of a player?
//! OK, we also add a `Hunger` component for player entities.
//! Now, what if we have an unknown number of such attributes,
//! determined at runtime (e.g. by plugins or an authoritative game server)?
//! Since we cannot declare types dynamically, it seems we have to refactor into a map or a Vec.
//! Or maybe a `SmallVec<[Attribute; N]>` to avoid heap allocation.
//! Wait, how much is `N`?
//!
//! In dynec, we avoid this problem with "isotope components".
//! Similar to isotopes in chemistry,
//! there may be multiple components of the same type (`Attribute`) for the same entity,
//! but these components belong to different "discriminants" (e.g. the attribute ID).
//! So in terms of semantics, it looks as if we got a `HashMap<AttributeId, Attribute>` component,
//! but in terms of performance, each `AttributeId` gets allocated in a new storage
//! as if it is a different component.
//! This design is also more efficient in the example here,
//! because some systems may only want to manipulate health but not hunger,
//! so it should be able to execute concurrently with systems that use the hunger attribute;
//! it is also better for cache locality since it avoids striding attributes with unused values.
//! This is not possible in ECS frameworks that only support type-based component key,
//! which lack flexibility for dynamically defined logic.
//!
//! # Entities can be rearranged to optimize random access (not yet implemented)
//! One of the reasons why ECS performs better than traditional OOP-based code style
//! is that components are stored in a compact region instead of scattered around the heap,
//! reducing the frequency of CPU cache penetration that causes slow memory access.
//! However, when the amount of data is large,
//! since entities are typically randomly arranged (no less random than heap allocation),
//! systems may need to access components from entities arranged far apart.
//! For example, in the case of iterating over all edges in a network simulation
//! (where nodes and edges are entities, and edges have components referencing the endpoint nodes),
//! although the data describing the edge itself are contiguously arranged,
//! accessing the data for the endpoint nodes would lead to random memory access,
//! greatly deterriorating the performance.
//!
//! In dynec, since all entity references are trackable,
//! it is possible to permute all entities of the same archetype
//! so that relevant entities are located more closely.
//! For example, in the case of a spatial graph
//! (where edge length is comparable to node density, i.e. very few super-long edges),
//! we can perform an quadtree/octree sort on all nodes and edges such that
//! iterating over all edge entities would process spatially nearby edges,
//! which in turn accesses spatially nearby nodes,
//! both of which have higher chance of getting nearby memory allocation.
//!
//! Of course, entity rearrangement is only useful for scenarios
//! where the ideal entity arrangement can be retained for a long period.
//! For example, it is useful to rearrange buildings on a map because they are mostly stationary,
//! but it is not useful to rearrange cars travelling on the map since their order quickly changes
//! (unless cars have very slow speed or move in a similar direction as nearby cars).
//! Since entity rearrangement requires mutable access to all component storages for an archetype
//! and processes a lot of data at the same time,
//! this is a stop-the-world operation that must not be performed frequently,
//! so the period for which the arrangement drifts away (such that rearrangement is necessary)
//! should be negligibly long such that user experience is not affected.
//!
//! [k8s-finalizer]: https://kubernetes.io/docs/concepts/overview/working-with-objects/finalizers/

#![cfg_attr(debug_assertions, allow(dead_code, unused_variables))]
#![cfg_attr(not(debug_assertions), deny(missing_docs))]
#![cfg_attr(
    not(any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    )),
    allow(dead_code)
)]
#![cfg_attr(doc, warn(missing_docs))]
#![feature(impl_trait_in_assoc_type)]
#![feature(maybe_uninit_uninit_array, maybe_uninit_array_assume_init)]
#![feature(never_type)]
#![feature(sync_unsafe_cell)]

/// Internal re-exports used in macros.
#[doc(hidden)]
pub mod _reexports {
    pub use {static_assertions, xias};
}

mod macros;
#[doc(inline)]
pub use macros::*;

#[macro_use]
pub mod tracer;

pub mod archetype;
pub use archetype::Archetype;

pub mod comp;

pub mod entity;
pub use entity::Entity;

mod global;
pub use global::Global;

pub mod scheduler;

pub mod storage;
pub use storage::Storage;

pub mod system;

#[cfg(any(test, feature = "internal-bench"))]
pub mod test_util;

pub mod world;
pub use world::{new, Bundle, World};

pub mod util;
