# Isotope Components

Sometimes we want to store multiple components of the same type on an entity.
For example, we want to store the ingredients that make up a bullet.
The straightforward approach is to use
a `Vec<(Element, Weight)>`/`HashMap<Element, Weight>`,
but this is very bad for performance and memory due to many heap allocations,
making ECS almost as slow as OOP.

Isotope components allow us to create components dynamically.
While simple components are identified by their type,
isotope components are identified by the type along with a "discriminant" value,
which is an (optionally newtyped) `usize` that distinguishes between isotopes.
For example, in the example above,
`Element` can be used as the discriminant
that distinguishes between different "weight" components,
such that each `Weight` component refers to a different element.

Like simple components, isotope components are also archetyped,
but they implement [`comp::Isotope<A>`][comp.isotope] instead,
which can also be achieved through the `#[comp]` macro:

```rust
#[comp(of = Bullet, isotope = Element)]
struct Ingredient(Weight);
```

## Choosing the discriminant type

Since a new component storage is created for every new isotope discriminant,
the number of different discriminants must be kept finite.
An example valid usage is to have each discriminant
correspond to one item defined in the game config file,
which is a realistically small number that does not grow with the game over time.

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

[comp.isotope]: https://sof3.github.io/dynec/master/dynec/comp/trait.Isotope.html
