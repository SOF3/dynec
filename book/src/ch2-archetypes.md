# Archetypes

In traditional ECS, the type of an entity is identified by the components it has.
For example, an entity is considered to be "moveable"
if it has the "location" and "speed" components.
Systems iterate over the "moveable" entities by performing a "join query"
that intersects the entities with a "location" and the entities with a "speed".
Thus, an entity type is effectively a subtype of any combination of its components,
e.g. both "player" and "bullet" are subtypes of "moveable".

Dynec takes a different approach on entity typing.
Dynec requires the type of an entity (its "archeytpe") to be
*known* during creation and *immutable* after creation ("statically archetyped").
A reference to an entity always contains the archetype.

Dynec still supports adding/removing components for an entity,
but this is implemented by making the component optional (effectively `Option<Comp>`)
instead of changing its archetype.
Adding/removing a component would not affect
systems iterating over all entities of its archetype.

To iterate over entities with only a specific component,
the suggested approach is to split the components
to a separate entity with a new archetype
and iterate over entities with that archetype instead.
(It is also possible to iterate over entities with a specific component,
but it is less efficient than iterate over all entities of the same component,
and joining multiple components is not supported)

Archetypes are typically represented as an unconstructable type (an empty enum)
referenced as a type parameter in system declarations.
Therefore, multiple systems can reuse the same generic function
where the archetype is a type parameter,
achieving something similar to the "subtyping" approach.
Nevertheless, Dynec discourages treating archetypes as subtypes
and encourages splitting shared components to an entity.
Therefore, it is possible to reuse the same function
for multiple systems by leaving the archetype as a type parameter.

An archetype can be declared through the [`dynec::archetype`][macro.archetype] macro:

```rust
dynec::archetype! {
    /// A building entity can store goods inside.
    pub Building;

    /// Each road entity represents a connection between two buildings.
    pub Road;
}
```

There is nothing magical here;
each line just declares an empty enum and implements [`Archetype`][trait.archetype] for it.

[macro.archetype]: ../dynec/macro.archetype.html
[trait.Archetype]: ../dynec/archetype/trait.Archetype.html
