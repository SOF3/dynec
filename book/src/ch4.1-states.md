# Parameter, local and global states

## Parameter states

A system may request parameters when building:

```rust
#[system]
fn hello_world(
    #[dynec(param)] counter: &mut i32,
) {
    *counter += 1;
    println!("{counter}");
}

builder.schedule(hello_world.build(123));
builder.schedule(hello_world.build(456));

// ...
world.execute(dynec::tracer::Noop); // prints 124 and 457 in unspecified order
world.execute(dynec::tracer::Noop); // prints 125 and 458 in unspecified order
```

The parameter type must be a reference (`&T` or `&mut T`) to the actual stored type.

Each `#[dynec(param)]` parameter in `hello_world`
must be a reference (`&T` or `&mut T`),
adds a new parameter of type `T`
to the generated `build()` method in the order they are specified,
with the reference part stripped.

Parameter states, along with other states, may be mutated when the system is run.
Each system (each instance returned by `build()`) maintains its own states.

## Local states

Unlike parameter states, local states are defined by the system itself
and is not specified through the `build()` function.

```rust
#[system]
fn hello_world(
    #[dynec(local(initial = 0))] counter: &mut i32,
) {
    *counter += 1;
    println!("{counter}");
}

builder.schedule(hello_world.build());
builder.schedule(hello_world.build());

// ...
world.execute(dynec::tracer::Noop); // prints 1, 1 in unspecified order
world.execute(dynec::tracer::Noop); // prints 2, 2 in unspecified order
```

`0` is the initial value of `counter` before the system is run the first time.
If parameter states are defined in the function,
the `initial` expression may use such parameters by name as well.

## Global states

States can also be shared among multiple systems
using the type as the identifier.
Such types must implement the [`Global`][trait.Global] trait,
which can be done through the [`#[global]`][attr.global] macro:

```rust
#[derive(Default)]
#[dynec::global(initial = Self::default())]
struct MyCounter {
    value: i32,
}

#[system]
fn add_counter(
    #[dynec(global)] counter: &mut MyCounter,
) {
    counter.value += 1;
}

#[system]
fn print_counter(
    #[dynec(global)] counter: &MyCounter,
) {
    println!("{counter}");
}
```

If no `initial` value is specified in `#[global]`,
the initial value of a global state must be assigned
in [`Bundle::register`][Bundle::register].

```rust
impl world::Bundle for Bundle {
    fn register(&mut self, builder: &mut world::Builder) {
        builder.schedule(add_counter.build());
        builder.schedule(print_counter.build());
        builder.global(MyCounter { value: 123 });
    }
}

// ...
world.execute(dynec::tracer::Noop); // prints 123 or 124 based on unspecified order
world.execute(dynec::tracer::Noop); // prints 124 or 125 based on unspecified order
```

The program panics if some used global states do not have an `initial`
but `Bundle::register` does not initialize them.

Note that `&T` and `&mut T` are semantically different for global states.
Multiple systems requesting `&T` for the same `T` may run in parallel
in a multi-threaded runtime,
but when a system requesting `&mut T` is running,
all other systems requesting `&T` or `&mut T` cannot run until the system is complete
(but other unrelated systems can still be scheduled).

[trait.Global]: ../dynec/trait.Global.html
[attr.global]: ../dynec/attr.global.html
[Bundle::register]: ../dynec/world/trait.Bundle.html#method.register
