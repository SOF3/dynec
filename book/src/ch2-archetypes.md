# Archetypes

In traditional ECS frameworks, entities are grouped by the components they have,
e.g. if an entity has a "location" and "speed" component,
it is considered as an entity that can move and
is looped by the system that requests these components.

In dynec, entities are *statically archetyped*,
which means the possible components of an entity is *known* and *fixed* from creation.
In the analogy of rows and columns, an archetype is similar to a table.

What if we want to add/remove components for an entity?
dynec still supports optional components,
but the entity is still stored in the same archetype,
so it still appears in the loop when systems iterate over this archetype.
If you would like to loop over entities with certain components,
it is a better idea to split the components to a separate entity with a new archetype
and loop on that archetype instead.

Declaring an archetype is very simple.
In this book, we will implement a logistics simulator step by step.
Let's represent "buildings" and the "roads" between them as entities:

```rust
dynec::archetype! {
    /// A building entity can store goods inside.
    pub Building;

    /// Each road entity represents a connection between two buildings.
    pub Road;
}
```
