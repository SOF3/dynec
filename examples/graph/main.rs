//! A basic flow simulation.
//!
//! The intended flowchart can be found in `flow.dot`.

#![allow(dead_code)]

mod render;
mod simulation;
use std::collections::HashMap;

use dynec::tracer;
use simulation::{Capacity, Node, CROPS};

use crate::simulation::WhichNode;
mod time;

fn main() {
    let mut world = dynec::new([
        Box::new(render::Bundle) as Box<dyn dynec::Bundle>,
        Box::new(time::Bundle),
        Box::new(simulation::Bundle),
    ]);

    // We can get components directly from the world when systems are not executing,
    // but this should only be used for test assertions.

    // Here, we first collect the entities to a HashMap according to their WhichNode component,
    // because we cannot iterate and get at the same time.
    // This limitation only applies to offline mode;
    // in an online system, use the EntityIterator API.
    let storage = world.components.get_simple_storage::<Node, WhichNode>();
    let nodes: HashMap<_, _> =
        storage.iter().map(|(entity, which)| (*which, world.rctrack.to_strong(entity))).collect();

    let crops_in_farm = world
        .components
        .get_isotope::<Node, Capacity, _>(nodes.get(&WhichNode::Farm).unwrap(), CROPS);
    assert_eq!(crops_in_farm, Some(&mut Capacity(100)));

    world.execute(&tracer::Noop);
}
