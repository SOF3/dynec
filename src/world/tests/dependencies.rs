//! Tests autoinit dependencies.

use crate::test_util::*;
use crate::{system, system_test};

#[system(dynec_as(crate))]
fn common_test_system(
    _comp3: system::ReadSimple<TestArch, Simple3OptionalDepends12>,
    _comp4: system::WriteSimple<TestArch, Simple4Depends12>,
    _comp5: system::ReadSimple<TestArch, Simple5RequiredNoInit>,
    _comp6: system::ReadSimple<TestArch, Simple6RequiredWithInitNoDeps>,
    #[dynec(isotope(discrim = [TestDiscrim1(11), TestDiscrim1(17)]))]
    _iso1: system::ReadIsotopePartial<TestArch, IsoNoInit, [TestDiscrim1; 2]>,
    #[dynec(global)] _aggregator: &mut Aggregator,
    #[dynec(global)] _initials: &InitialEntities,
) {
}

#[test]
fn test_dependencies_successful() {
    let mut world = system_test!(common_test_system.build(););
    let entity = world.create::<TestArch>(crate::comps![ @(crate) TestArch =>
        Simple1OptionalNoDepNoInit(1), Simple5RequiredNoInit(1),
        @(TestDiscrim1(11), IsoNoInit(1)),
        @(TestDiscrim1(13), IsoNoInit(2)),
        @(TestDiscrim1(17), IsoNoInit(3)),
    ]);

    match world.components.get_simple_storage::<TestArch, Simple4Depends12>().try_get(&entity) {
        Some(&Simple4Depends12(c40, c41)) => {
            assert_eq!(c40, 7);
            assert_eq!(c41, (1 + 2) * 8);
        }
        None => panic!("Simple4Depends12 is used in system_with_comp3_comp4_comp5"),
    }

    world.components.get_simple_storage::<TestArch, Simple2OptionalDepends1>(); // should not panic
}

#[test]
#[should_panic = "Cannot create an entity of type `dynec::test_util::TestArch` without explicitly \
                  passing a component of type \
                  `dynec::test_util::simple_comps::Simple5RequiredNoInit`"]
fn test_dependencies_missing_required_simple() {
    let mut world = system_test!(common_test_system.build(););
    world.create::<TestArch>(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(1)]);
}

#[test]
#[should_panic = "Cannot create an entity of type `dynec::test_util::TestArch` without explicitly \
                  passing a component of type \
                  `dynec::test_util::simple_comps::Simple2OptionalDepends1`, or \
                  `dynec::test_util::simple_comps::Simple1OptionalNoDepNoInit` to invoke its \
                  auto-initializer"]
fn test_dependencies_missing_required_dep() {
    let mut world = system_test!(common_test_system.build(););
    world.create::<TestArch>(crate::comps![@(crate) TestArch => Simple5RequiredNoInit(1)]);
}
