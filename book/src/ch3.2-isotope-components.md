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

As mentioned above, isotope components are just like simple components with the type
`HashMap<Discriminant, Value>`.
Initializers for isotope components return
iterators of (discriminant, value) tuples instead.

Since the returned iterator involves dynamic discriminant values,
it is not possible to implement [`comp::Must`][must] for isotope components automatically.
Nevertheless, if the user is sure that all discriminants are populated
in the initializer through exhausting the domain of discriminants,
they can implement this trait manually.

[comp.isotope]: https://sof3.github.io/dynec/master/dynec/comp/trait.Isotope.html
[must]: https://sof3.github.io/dynec/master/dynec/comp/trait.Must.html
