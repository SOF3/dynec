# Component access

As the name "ECS" implies,
the most important feature is to access the "E" and "C" from the "S".

## Accessing simple components

Simple components can be accessed with [`ReadSimple`][read.simple] or [`WriteSimple`][write.simple].
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
    mut position_acc: impl system::WriteSimple<Bullet, Position>,
    velcity_acc: impl system::ReadSimple<Bullet, Velocity>,
) {

}
```

[read.simple]: https://sof3.github.io/dynec/master/dynec/system/trait.ReadSimple.html
[write.simple]: https://sof3.github.io/dynec/master/dynec/system/trait.WriteSimple.html
