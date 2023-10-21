# Component access

As the name "ECS" implies,
the most important feature is to manipulate the "E" and "C" from the "S".

## Accessing simple components

Simple components can be accessed through [`ReadSimple`][ReadSimple] or [`WriteSimple`][WriteSimple].
First we declare the components we need, similar to in the previous chapters:

```rust
use dynec::{comp, system};

dynec::archetype!(Bullet);

#[comp(of = Bullet, required)]
struct Position(Vector3<f32>);
#[comp(of = Bullet, required, initial = Velocity(Vector3::zero()))]
struct Velocity(Vector3<f32>);
```

We want to update position based on the value of the velocity.
Therefore we request reading velocity and writing position:

```rust
#[system]
fn motion(
    mut position_acc: system::WriteSimple<Bullet, Position>,
    velocity_acc: system::ReadSimple<Bullet, Velocity>,
) {
    // work with position_acc and velocity_acc
}
```

We will go through how to work with the data later.

When a system that requests `WriteSimple<A, C>` is running for some `A` and `C`,
all other systems that request `ReadSimple<A, C>` or `WriteSimple<A, C>`
cannot run until the system is complete.
Therefore, if you only need to read the data,
use `ReadSimple` instead of `WriteSimple` even though
the latter provides all abilities that the former can provide.

## Accessing isotope components

Isotope components are slightly more complex.
A system may request access to
some ("partial access") or all ("full access") discriminants for an isotope component.

Full access allows the system to read/write any discriminants for the isotope type,
and lazily initializes new discriminants if they were not encountered before.
Therefore, when a system using `WriteIsotopeFull` is running,
all other systems that access the same component in any way (read/write and full/partial)
cannot run until the system is complete;
when a system using `ReadIsotopeFull` is running,
all other systems that use `WriteIsotopeFull` or `WriteIsotopePartial`
on the same component cannot run until the system is complete.

The usage syntax of full accessors is similar to simple accessors:

```rust
#[system]
fn add(
    weights: ReadIsotopeFull<Bullet, IngredientWeight>,
    mut volumes: WriteIsotopeFull<Bullet, IngredientVolume>,
) {
    // ...
}
```

Partial access only requests specific discriminants for the isotope type.
The requested discriminants are specified through an attribute:

```rust
#[system]
fn add(
    #[dynec(param)] &element: &Element,
    #[dynec(isotope(discrim = [element]))]
    weights: ReadIsotopePartial<Bullet, IngredientWeight, [Element; 1]>,
    #[dynec(isotope(discrim = [element]))]
    mut volumes: WriteIsotopePartial<Bullet, IngredientVolume, [Element; 1]>,
) {
    // ...
}
```

The `discrim` attribute option lets us specify which discriminants to access.
The expression can reference the initial values of parameter states.
However, mutating parameter states will *not* change
the discriminants requested by the isotope.
The third type parameter to `ReadIsotopePartial`/`WriteIsotopePartial`
is the type of the expression passed to `discrim`.

Since a partial accessor can only interact with specific discriminants,
multiple systems using `WriteIsotopePartial` on the same component type
can run concurrently if they request a disjoint set of discriminants.

## Iterating over entities

The recommended way to process all entities with accessors is
to use the [`EntityIterator`][EntityIterator] API.
`EntityIterator` contains the list of initialized entities
stored in an efficient lookup format,
useful for performing bulk operations over all entities.

An `EntityIterator` can be joined with multiple accessors
to execute code on each entity efficiently:

```rust
#[system]
fn move_entities(
    entities: system::EntityIterator<Bullet>,
    position_acc: system::WriteSimple<Bullet, Position>,
    velocity_acc: system::WriteSimple<Bullet, Velocity>,
) {
    for (_entity, (position, velocity)) in entities.entities_with_chunked((
        &mut position_acc,
        &velocity_acc,
    )) {
        *position += velocity;
    }
}
```

`entities_with_chunked` also supports isotope accessors,
but they must be split for a specific discriminant first
by calling `split` on the accessor (`split_mut` for mutable accessors):

```rust
#[system]
fn move_entities(
    #[dynec(param)] &element: &Element,
    entities: system::EntityIterator<Bullet>,
    velocity_acc: system::WriteSimple<Bullet, Velocity>,
    #[dynec(isotope(discrim = [element]))]
    weights_acc: system::ReadIsotopePartial<Bullet, IngredientWeight, [Element; 1]>,
) {
    let [weights_acc] = weights_acc.split([element]);
    entities
        .entities_with_chunked((
            &mut velocity_acc,
            &weights_acc,
        ))
        .for_each(|(_entity, (velocity, weight))| {
            *velocity /= weight;
        }
    }
}
```

> Note: `entities_with_chunked` returns an iterator,
> so you may use it with a normal `for` loop as well.
> However, benchmarks show that `for_each` has performs
> significantly better than `for` loops due to vectorization.

You may also use `par_entities_with_chunked` instead
to execute the loop on multiple threads.
`par_entities_with_chunked` returns a rayon [`ParallelIterator`][rayon::ParallelIterator],
which has a very similar API to the native `Iterator`.

[ReadSimple]: ../dynec/system/type.ReadSimple.html
[WriteSimple]: ../dynec/system/type.WriteSimple.html
[EntityIterator]: ../dynec/system/iter/struct.EntityIterator.html
[rayon::ParallelIterator]: ../rayon/iter/trait.ParallelIterator.html
