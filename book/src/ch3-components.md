# Components

Components store the actual data for an entity.
In Dynec, since entities are statically archetyped,
a component is only meaningful when specified togethre with an archetype.

There are two kinds of components, namely "simple components" and "isotope components".
For simple components, each entity cannot have
more than one instance for each component type.
Meanwhile, isotope components allow storing multiple instances
of the same component type for the same entity.
