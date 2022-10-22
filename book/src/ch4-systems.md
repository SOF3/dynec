# Systems

Systems contain the actual code that process components.

Systems can be easily created using the [`#[system]`][macro.system] macro:

```rust
use dynec::system;

#[system]
fn hello_world() {
    println!("Hello world!");
}
```

After the `#[system]` macro is applied,
`hello_world` becomes a unit struct
with the associated functions `hello_world::call()` and `hello_world.build()`.
`call` calls the original function directly,
while `build()` creates a system descriptor that can be passed to a world builder.

We can package this system into a "bundle"

```rust
use dynec::world;

pub struct MyBundle;

impl world::Bundle for Bundle {
    fn register(&mut self, builder: &mut world::Builder) {
        builder.schedule(Box::new(hello_world.build()));
        // schedule more systems here
    }
}
```

Then users can add the whole bundle into their world:

```rust
let mut world = dynec::new([
    Box::new(MyBundle),
    // add more bundles here
]);
```

Alternatively, for non-distribution scenarios (e.g. unit testing),
the [`system_test!`][system_test] macro can be used:

```rust
let mut world = dynec::system_test!(
    hello_world.build();
);
```

Calling `world.execute()` would execute the world once.
Run this in your program main loop:

```rust
event_loop.run(|| {
    world.execute(&dynec::tracer::Noop);
})
```

Since dynec is just a platform-agnostic ECS framework,
it does not integrate with any GUI frameworks to execute the main loop directly.
Usually it is executed at the same rate as the world simulation, screen rendering
or turns (for turn-based games), depending on your requirements.

[macro.system]: https://sof3.github.io/dynec/master/dynec/attr.system.html
[system_test]: https://sof3.github.io/dynec/master/dynec/macro.system_test.html
