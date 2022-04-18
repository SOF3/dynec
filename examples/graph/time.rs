//! This module simply defines a sytem that uses a Global to measure delta-time.
//! Nevertheless, for the sake of testing consistency,
//! we use a `Box<dyn Fn() -> u64>` to measure the time
//! instead of calling `std::time::Instant::elapsed()`.

/// The main API of this module.
pub struct Bundle;

impl dynec::world::Bundle for Bundle {
    /// Initializes the plugin, registering systems and initializing globals.
    fn register(&self, builder: &mut dynec::world::Builder) {
        // builder.schedule(tick);
    }

    /// Populates the world with entities.
    /// In actual games, this function should load the world from a save file instead.
    fn populate(&self, _: &mut dynec::World) {}
}

// Normally this should be a `std::time::Instant`, but we use `u64` for mocking.
type Time = u64;

#[derive(dynec::EntityRef)]
pub struct TimeFunction {
    f: Box<dyn Fn() -> Time>,
}

impl dynec::Global for TimeFunction {}

#[derive(dynec::EntityRef)]
pub struct Delta {
    pub delta: Time,
}
impl dynec::Global for Delta {}

/*
#[dynec::system]
fn tick(#[local] previous: &mut Time, time_fn: &TimeFunction, delta: &mut Delta) {
    use dynec::system::Context;

    let now = (time_fn.f)();
    delta.delta = now - previous.previous;
    *previous = now;
}
*/
