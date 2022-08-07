# dynec
An opinionated ECS framework.

[![CI](https://github.com/SOF3/dynec/actions/workflows/ci.yml/badge.svg)](https://github.com/SOF3/dynec/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/SOF3/dynec/branch/master/graph/badge.svg?token=WAU2FOLHVW)](https://codecov.io/gh/SOF3/dynec)

## Design goals (i.e. opinions)
### Leverage type checker
The design goal of dynec is to leverage compile-time type checking
to reveal as many bugs as possible without requiring tests.
Therefore, compile-time-only type parameters are extensively used throughout the project.

### Maximize cache locality
dynec attempts to maximize cache locality
by providing the ability to permute the order of entities on demand,
allowing the user to sort nearby entities together.

## Features (or anti-features)
### Explicit static archetypes.
- The archetype of an entity is fixed at the time of creation.
- Entities cannot change archetypes.
  - Components can be *optional* or *required*/*auto-init*.
  - Required/auto-init components always exist for an entity of that archetype,
    so no need to [`Option::unwrap`] all the time.
- Entities of different archetypes are stored separately.
  - Yes, only components of the same type and the same archetype are stored together.
  - This improves cache locality since components for the same archetype
    are more likely to be processed together.
- Entity references are typed by archetype.
  - This allows ensuring at compile time that an entity always has a certain component.
    Elides runtime checks once again.

### Safe deletion
- Entity deletion is deferred until all "finalizer" components are removed,
  similar to `.metadata.finalizers` in Kubernetes.
- When debug assertions are enabled,
  entity deletion panics if it is still referenced from other entities.
  - Dangling entity references are detected as soon as possible.
  - This is implemented by conditionally storing an `Arc<()>` in debug mode.

### Isotope components
- Motivation: sometimes we want a finite, small but dynamic number of components of the same
type for the same entity.
- This is typically implemented with `Vec<T>`/`SmallVec<[T; N]>`,
  - But it is difficult to determine `N` at compile time.
  - All `T` are closely packed together, but perhaps we want to pack `T`s of the same index
    together instead for better cache locality.
- Isotope components are treated as components with the type `(TypeId::of::<T>(), discriminant)`
- The discriminant is a small number similar to what you would use in a `SmallVec<[T; N]>`.
- Expected use case: Each discriminant is a relatively independent system where components with
  that discriminant interact closely.

### Entity rearrangement
- Motivation: entities may have mostly static locations in the game world, so it is useful to
sort all entities by location to improve cache locality when nearby entities interact.
- Provides a permutation API to rearrange entities of a certain archetype.
- Entity references are updated immediately after rearrangement.
  - This requires all state and component types to implement a trait that iterates over all
    entity references.
  - If this was implemented incorrectly, it results in a panic in debug mode through
    refcounting.
- Rearrangement is a stop-the-world operation that happens between ticks, so perform sparingly.
- It is not useful for entities that rearrange frequently, e.g. players that travel around
  frequently.
- It is useful for entities that are mostly stationary, e.g. buildings that cannot move.
