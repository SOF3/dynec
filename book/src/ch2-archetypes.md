# Archetypes

In traditional ECS frameworks, entities are grouped by the components they have,
e.g. if an entity has a "location" and "speed" component,
it is considered as an entity that can move and
is looped by the system that requests these components.
The set of components created for an entity is called its "archetype",
which is comparable to "classes" in OOP.

In dynec, entities are *statically archetyped*,
which means the possible components of an entity is *known* and *fixed* from creation.
In the analogy of rows and columns, an archetype is similar to a table.
As such, different archetypes have their own entity IDs.

What if we want to add/remove components for an entity?
dynec still supports optional components,
but the entity is still stored in the same archetype,
so it still appears in the loop when systems iterate over this archetype.
If you would like to loop over entities with certain components,
it is a better idea to split the components to a separate entity with a new archetype
and loop on that archetype instead.
(It is also possible to loop over entities with a specific component,
but joining multiple components is not supported)

Archetypes are typically represented as an unconstructable type (an empty enum)
that is referenced as a type parameter in system declarations.
Therefore, it is possible to reuse the same function
for multiple systems by leaving the archetype as a type parameter.
There is a convenience macro to achieve this:

```rust
use dynec::archetype;

archetype! {
    /// A building entity can store goods inside.
    pub Building;

    /// Each road entity represents a connection between two buildings.
    pub Road;
}
```

The [`archetype!` macro][macro.archetype] just declares an empty enum
that implements [`Archetype`][trait.archetype].

[macro.archetype]: https://sof3.github.io/dynec/master/dynec/macro.archetype.html
[trait.archetype]: https://sof3.github.io/dynec/master/dynec/archetype/trait.Archetype.html
