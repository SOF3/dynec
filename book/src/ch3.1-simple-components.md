# Simple components

Simple components are components where
each entity can have at most one instance of the component.
A type can be used as a simple component for entities of archetype `A`
if it implements [`comp::Simple<A>`][comp::Simple].
Dynec provides [a convenience macro][macro.comp] to do this:

```rust
#[comp(of = Bullet)]
struct Location([f32; 3]);
```

This declares a simple component called `Location`
that can be used on `Bullet` entities.

The same type can be reused as components for multiple archetypes.
by applying the macro multiple times:

```rust
use dynec::comp;

#[comp(of = Player)]
#[comp(of = Bullet)]
struct Location([f32; 3]);
```

## Initializer

Simple components can be equipped with an auto-initializer.
If an entity is created without specifying this component,
the auto-initializer is called to fill the component.

The auto-initializer can read values of other simple components,
either specified by the entity creator or returned by another auto-initializer.
Since Dynec does not persist a component
unless it is requested by a system or explicitly registered,
this means you can pass a temporary component during entity creation,
use its value in other component auto-initializers,
and this temporary component gets dropped after entity creation completes.

The auto-initializer can be specified in the macro
either as a closure:

```rust
use dynec::comp;

#[comp(of = Bullet, init = |velocity: &Velocity| Damage(velocity.norm()))]
struct Damage(f32);
```

or as a function pointer with arity notation
(i.e. write the number of parameters for the function after a `/`):

```rust
use dynec::comp;

fn default_damage(velocity: &Velocity) -> Damage {
    Damage(velocity.norm()) 
}

#[comp(of = Bullet, init = default_damage/1)]
struct Damage(f32);
```

## Presence

A component is either `Required` or `Optional`.

`Optional` components may be missing on some entities.
Accessing optional components returns `Option<C>` instead of `C`.

`Required` components must either have an auto-initializer
or be passed during entity creation.
This ensures that accessing the component always succeeds for an initialized entity;
optimizations such as chunk iteration are only possible for `Required` components.
Nevertheless, components are **always** missing
for uninitialized entities created during the middle of a tick;
more will be explained in later sections.

A `Required` component must *both*
set [`PRESENCE = SimplePresence::Required`][comp::Simple::PRESENCE]
*and* implement [`comp::Must<A>`][Must].
This is automatically done by specifying `required` in the `#[comp]` macro:

```rust
use dynec::comp;

#[comp(of = Bullet, required)]
struct Damage(u32);
```

## Finalizers

A finalizer component is a component that prevents an entity from getting deleted.

> Yes, I know this may be confusing.
> Contrary to finalizers in Java/C\#,
> a finalizer is a data component instead of a function.
> They are actually more similar to [finalizers in Kubernetes][k8s-finalizers].

When an entity is flagged for deletion,
Dynec checks if all finalizer components for that entity have been removed.
If there is at least one present finalizer component for the entity,
the entity would instead be scheduled to asynchronously delete
when all finalizer components have been unset.

This gives systems a chance to execute cleanup logic
by reading the component data of the "terminating" entity.
For example, a system that despawns deleted bullets from network players
may get a chance to handle bullet deletion:

```text
for each `Bullet` entity flagged for deletion:
    if `Despawn` componnent is set
        read component `NetworkId` for the entity
        broadcast despawn packet to all players
        unset the `Despawn` finalizer component
```

Without the finalizer component,
the system would be unable to get the `NetworkId` for the despawned bullet
since the component has been cleaned up.

Note that deletion-flagged entities are checked every tick.
To avoid a growing backlog of entities to delete,
finalizer components should be removed as soon as possible
after deletion has been flagged.

## Best practices

### Small component structs

Dynec prevents systems that write to the same component type
from executing concurrently to avoid data race.
In reality, most systems only need to access a subset of fields,
so avoid putting many unrelated fields in the same component type.
Instead, prefer small, often single-field structs,
unless the multiple fields are naturally related,
e.g. positions/RGB values that are always accessed together.

### Optional types

Avoid using [`Option`][option] in component types;
instead, use optional components to represent unused fields.
By default, Dynec uses a compact bit vector to track the existence of components,
which only takes 1 bit per component.
Meanwhile, `Option<T>` needs to preserve the alignment of `T`,
so a type like `Option<f64>` is 128 bits large
(1 bit for `None`, 63 bits for alignment padding, 64 bits for the actual data),
which is very wasteful of memory.

### Heap-allocated types

Minimize external (heap) memory referenced in entity components.
Heap allocation/deallocation is costly,
and the memory allocated is randomly located in the memory,
which means the CPU needs to keep loading new memory pages
into its memory cache layers
and greatly worsens performance.
Dynec stores component data in (almost) contiguous memory
and prefers processing adjacent entities in the same CPU,
so keeping all relevant data in the component structure is preferred.

While this is inevitable for some component types like strings,
types like `Vec` can often be avoided:

- If each entity has a similar structure of items
  (i.e. `comp[0]` for entity 1 has the same logic as `comp[0]` for entity 2),
  use isotope components instead.
- If the items in the vector are unstructured
  (i.e. `comp[0]` for entity 1 has the same logic as `comp[1]` for entit y2),
  consider turning each item into an entity and process the entity instead.

[comp::Simple]: https://sof3.github.io/dynec/master/dynec/comp/trait.Simple.html
[comp::Simple::PRESENCE]: https://sof3.github.io/dynec/master/dynec/comp/trait.Simple.html#associatedconstant.PRESENCE
[macro.comp]: https://sof3.github.io/dynec/master/dynec/attr.comp.html
[Must]: https://sof3.github.io/dynec/master/dynec/comp/trait.Must.html
[k8s-finalizers]: https://kubernetes.io/docs/concepts/overview/working-with-objects/finalizers/
[option]: https://doc.rust-lang.org/std/option/enum.Option.html
