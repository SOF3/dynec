# Simple components

Simple components are components where
each entity can have at most one instance of the component.
To declare that a type `C` is a component for entities of archetype `A`,
implement [`comp::Simple<A>`][comp.simple] for `C`.
dynec provides a [convenience macro][macro.comp] for this:

```rust
#[comp(of = Bullet)]
struct Location([f32; 3]);
```

Since Rust allows implementing the same trait with different type parameters,
`C` can be used as a component for entities of both `A` and `B`
if it implements `comp::Simple<A>` and `comp::Simple<B>` separately.
This can be achieved by invoking the macro twice:

```rust
use dynec::comp;

#[comp(of = Player)]
#[comp(of = Bullet)]
struct Location([f32; 3]);
```

Note that components are only stored on entities if at least one system uses it.

## Initializer

Simple components can be equipped with an auto-initializer.
When an entity is created without passing this component,
it is filled with the value returned by the auto-initializer.

The auto-initializer can depend on other simple components
passed by the entity creator or other auto-initializers.
Along with the fact that only components requested from systems get persisted,
this means you can pass a parameter during entity creation,
let other component auto-initializers read from this parameter,
and this parameter will get dropped after entity creation completes.

The auto-initializer can be specified in the macro
either as a closure:

```rust
use dynec::comp;

#[comp(of = Bullet, init = |speed: &Speed| Damage(speed.modulus()))]
struct Damage(f32);
```

or as a function pointer with arity (i.e. number of parameters for the function):

```rust
use dynec::comp;

fn default_damage(speed: &Speed) -> Damage {
    Damage(speed.modulus()) 
}

#[comp(of = Bullet, init = default_damage/1)]
struct Damage(f32);
```

## Presence

A component can be either "required" or "optional".

The "optional" presence allows components to be missing on some entities.
Therefore, accessing optional components only returns `Option<C>` instead of `C`.

"Required" components must either have an auto-initializer
or be passed during entity creation.
This ensures that accessing the component always succeeds for any entities.

Note that entities created during the middle of a tick
are only fully initialized after the end of the tick.
More will be explained in later sections.

Components with "required" presence should *both*
set [`PRESENCE = SimplePresence::Required`][comp.simple.presence]
*and* implement [`comp::Must<A>`][must].
This is automatically done by passing `required` in the macro:

```rust
use dynec::comp;

#[comp(of = Bullet, required)]
struct Damage(u32);
```

## Finalizers

A finalizer is a component that prevents an entity from getting deleted.

Yes, I know this may be confusing.
Contrary to finalizers in Java/C\#,
a finalizer is a data component instead of a function.
They are actually more similar to [finalizers in Kubernetes][k8s-finalizers].
When an entity is flagged for deletion,
dynec checks if all finalizer components for that entity have been removed;
the component data of the entity only get dropped after this check is true.

This gives systems a chance to execute cleanup logic
by reading the component data of the "terminating" entity.
For example, a system that despawns deleted bullets from network players
may implement its logic like the pseudocode below:

```
for each `Bullet` entity just flagged for deletion:
    read component `NetworkId` for the entity
    broadcast despawn packet to all players
    remove the `Despawn` finalizer component
```

Without the finalizer component,
the system would be unable to get the `NetworkId` for the despawned bullet.

Note that deletion-flagged entities are checked every tick.
To avoid impacting performance due to a growing backlog,
finalizer components should be removed as soon as possible
after deletion has been flagged.

## Choosing the component type

Systems that write to the same component type cannot execute together.
Furthermore, most games often need to access a single component over a loop,
so avoid putting multiple unrelated fields in the same component type.
Instead, prefer small, often single-field structs,
unless the multiple fields are naturally related,
e.g. positions/RGB values that are always accessed together.

Avoid using [`Option`][option] in component types;
instead, use optional components to represent unused fields.
dynec uses a compact bit vector to track the existence of components,
which only takes 1 bit for each component,
while `Option` needs to preserve alignment and could take up to 64 bits
if the wrapped type requires an alignment of 64 bits (e.g. `u64`/`f64`).

Avoid allocating heap memory for each entity component.
In other words, use of `Box`/`Vec`/etc should be avoided in component types,
because heap allocation is slow and results in memory fragmentation,
which greatly deterriorates the performance gain provided by ECS.

[comp.simple]: https://sof3.github.io/dynec/master/dynec/comp/trait.Simple.html
[comp.simple.presence]: https://sof3.github.io/dynec/master/dynec/comp/trait.Simple.html#associatedconstant.PRESENCE
[macro.comp]: https://sof3.github.io/dynec/master/dynec/attr.comp.html
[simple-presence.required]: https://sof3.github.io/dynec/master/dynec/comp/enum.SimplePresence.html#variant.Required
[simple-presence.optional]: https://sof3.github.io/dynec/master/dynec/comp/enum.SimplePresence.html#variant.Optional
[must]: https://sof3.github.io/dynec/master/dynec/comp/trait.Must.html
[k8s-finalizers]: https://kubernetes.io/docs/concepts/overview/working-with-objects/finalizers/
[option]: https://doc.rust-lang.org/std/option/enum.Option.html
