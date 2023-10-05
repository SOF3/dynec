//! Tests global state access.

use crate::test_util::*;
use crate::{global, system, system_test, tracer};

#[test]
fn test_global_update() {
    #[system(dynec_as(crate))]
    fn test_system(#[dynec(global)] aggregator: &mut Aggregator) { aggregator.comp30_sum = 1; }

    let mut world = system_test!(test_system.build(););

    world.execute(&tracer::Log(log::Level::Trace));

    let aggregator = world.get_global::<Aggregator>();
    assert_eq!(aggregator.comp30_sum, 1);
}

#[test]
#[should_panic = "Global type dynec::world::tests::globals::test_global_uninit::Uninit does not \
                  have an initial impl and was not provided manually"]
fn test_global_uninit() {
    #[global(dynec_as(crate))]
    struct Uninit;

    #[system(dynec_as(crate))]
    fn test_system(#[dynec(global)] _: &Uninit) {}

    let _world = system_test!(test_system.build(););
}
