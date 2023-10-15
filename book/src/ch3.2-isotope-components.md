# Isotope Components

Sometimes we want to store multiple components of the same type on an entity.
For example, we want to store the ingredients that make up a bullet.
The straightforward approach is to use
a `Vec<(Element, Ingredient)>`/`HashMap<Element, Ingredient>`,
but this is very bad for performance and memory due to many heap allocations.
This is where isotope components come handy.

An isotope component works like a component that stores
a map of "discriminants" to component values.
For example, in the example above,
`Element` can be used as the discriminant
that distinguishes between different "weight" components,
and an entity has a separate `Ingredient` for each `Element`.

Like simple components, isotope components are also archetyped,
but they implement [`comp::Isotope<A>`][comp::Isotope] instead,
which can also be achieved through the `#[comp]` macro:

```rust
#[derive(Discrim)]
struct Element(u16);

#[comp(of = Bullet, isotope = Element)]
struct Ingredient(f64);
```

Unlike vector/map simple components,
Dynec treats each discriminant as a different component
such that it has its own storage and lock mechanism,
so systems can execute in parallel
to process different discriminants of the same component.

## Choosing the discriminant type

Dynec creates a new component storage for every new isotope discriminant.
If you use the `storage::Vec` (the default) storage,
the space complexity is the product of
the number of entities and the number of possible discriminants.
Therefore, the number of possible discriminant values must be kept finite.

An example valid usage is to have each discriminant
correspond to one item defined in the game config file,
which is a realistically small number that does not grow with the game over time.
Ideally, the possible values of discriminant are generated from a 0-based auto-increment,
e.g. corresponding to the order of the item in the config file.

## Initializer

Similar to simple components, isotope components can also have an auto-initializer.
However, new discriminants may be introduced after entity creation,
so isotopes cannot be exhaustively initialized during entity creation
but initialized when new discriminants are added instead.
Therefore, isotope auto-initializers cannot depend on any other values.

## Presence

Isotope components can also have a `Required` presence like simple components.
However, since discriminants are dynamically introduced,
it is not possible to initialize an entity with all possible discriminants exhaustively.
An isotope component can be `Required` as long as it has an auto-initializer.

[comp::Isotope]: ../dynec/comp/trait.Isotope.html
