//! A basic flow simulation.
//!
//! The intended flowchart can be found in `flow.dot`.

#![allow(dead_code)]

mod render;
mod simulation;
mod time;

fn main() {
    let _world = dynec::new([
        Box::new(render::Bundle) as Box<dyn dynec::Bundle>,
        Box::new(time::Bundle),
        Box::new(simulation::Bundle),
    ]);

    // assert_eq!(world.get_multi::<Node, Capacity, _, _>(CROPS, farm, |x| *x), Some(Capacity(100)));
}
