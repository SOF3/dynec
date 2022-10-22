# Parameter, local and global states

## Local states

Systems can persist values over multiple executions,
known as "local states":

```rust
#[system]
fn hello_world(
    #[dynec(local(initial = 0))] counter: &mut i32,
) {
    *counter += 1;
    println!("counter = {counter}");
}
```

`0` is the initial value of `counter` before the system is run the first time.
The parameter type must be a reference (`&T` or `&mut T`) to the actual stored type.

Calling `world.execute()` in a row will print the following:

```text
counter = 1
counter = 2
counter = 3
...
```

## Parameter states

The initial value can be passed as a parameter instead:

```rust
#[system]
fn hello_world(
    #[dynec(param)] counter: &mut i32,
) {
    *counter += 1;
}

// ...

builder.schedule(Box::new(hello_world.build(123)));
```

The arguments to `.build()` are all `#[param]` parameters in the order they are defined.

## Global states

States can be shared between multiple systems, identified by their type.
Such types must implement the [`Global`][trait.global] trait,
which can be done through the [`#[global]`][attr.global] macro:

```rust
#[dynec::global(initial = Self::default())]
#[derive(Default)]
struct MyCounter {
    value: i32,
}

#[system]
fn hello_world(
    #[dynec(global)] counter: &mut MyCounter,
) {
    counter.value += 1;
}
```

The initial value of a global state can also be assigned
in [`Bundle::register`][bundle.register] instead
if it is not specified in the `#[dynec::global]`:

```rust
impl world::Bundle for Bundle {
    fn register(&mut self, builder: &mut world::Builder) {
        builder.schedule(Box::new(hello_world.build()));
        builder.global(MyCounter { value: 123 });
    }
}
```

The program panics if `Bundle::register` does not initialize all global states.

[trait.global]: https://sof3.github.io/dynec/master/dynec/trait.Global.html
[attr.global]: https://sof3.github.io/dynec/master/dynec/attr.global.html
[bundle.register]: https://sof3.github.io/dynec/master/dynec/world/trait.Bundle.html#method.register
