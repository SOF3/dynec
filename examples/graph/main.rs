//! A basic flow simulation.
//!
//! The intended flowchart can be found in `flow.dot`.

mod render;
mod simulation;
mod time;

fn main() {
    let _world = dynec::new([
        &render::Bundle as &dyn dynec::world::Bundle,
        &time::Bundle,
        &simulation::Bundle,
    ]);

    // assert_eq!(world.get_multi::<Node, Capacity, _, _>(CROPS, farm, |x| *x), Some(Capacity(100)));
}
