# Isotope Components

Unlike simple components,
there can be multiple instances of isotope components
of the same type for the same entity.
Multiple isotope components are distinguished
with a value called the "discriminant",
which is an (optionally newtyped) `usize`.

TODO: add example code

## Initializer

Since there can be an indefinite number of isotopes for the same type,
it is not possible to auto-initialize isotopes when an entity is created.
However, isotope components can be *lazily* initialized &mdash;
when a system requests the value of an isotope component that does not exist,
the initializer is called to create a new value.

Since isotope components are lazily initialized during the tick,
the current thread may not have access to other components of the entity.
Therefore, isotope lazy initializers cannot accept any parameters,
and only basic usage (e.g. returning the zero value) is expected.
Users who wish to initialize isotope components from other component values
have to do this manually from the accessing systems.

Due to thread safety, lazily initialized values requested from read-only systems
does not actually get stored with the entity after use.
Therefore, types with interior mutability (such as `AtomicUsize`)
would not work as expected.
Nevertheless, interior mutability is frowned upon for component data in general.
