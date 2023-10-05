//! Tests EntityIterator.

use crate::entity::{Raw as _, Ref};
use crate::test_util::*;
use crate::{system, system_test, tracer};

#[test]
fn test_entity_iter_partial_mut() {
    #[system(dynec_as(crate))]
    fn test_system(
        iter: system::EntityIterator<TestArch>,
        comp1_acc: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
        #[dynec(isotope(discrim = [TestDiscrim1(7), TestDiscrim1(13)]))]
        mut iso1_acc: system::WriteIsotopePartial<TestArch, IsoNoInit, [TestDiscrim1; 2]>,
        #[dynec(isotope(discrim = [TestDiscrim1(31)]))] mut iso1_acc_31: system::ReadIsotopePartial<
            TestArch,
            IsoNoInit,
            [TestDiscrim1; 1],
        >,
    ) {
        let [mut iso1_acc_0, mut iso1_acc_1] = iso1_acc.split_isotopes([0, 1]);
        let [iso1_acc_31] = iso1_acc_31.split([0]);

        for (entity, (comp1, iso10, iso11, iso131)) in iter.entities_with((
            system::Try(&comp1_acc),
            system::Try(&mut iso1_acc_0),
            system::Try(&mut iso1_acc_1),
            system::Try(&iso1_acc_31),
        )) {
            match entity.id().to_primitive() {
                1 => {
                    assert_eq!(comp1, Some(&Simple1OptionalNoDepNoInit(5)));
                    assert_eq!(iso10, Some(&mut IsoNoInit(11)));
                    assert_eq!(iso11, None);
                    assert_eq!(iso131, Some(&IsoNoInit(41)));
                }
                2 => {
                    assert_eq!(comp1, None);
                    assert_eq!(iso10, None);
                    assert_eq!(iso11, Some(&mut IsoNoInit(17)));
                    assert_eq!(iso131, Some(&IsoNoInit(43)));
                }
                3 => {
                    assert_eq!(comp1, None);
                    assert_eq!(iso10, Some(&mut IsoNoInit(19)));
                    assert_eq!(iso11, Some(&mut IsoNoInit(23)));
                    assert_eq!(iso131, None);
                }
                _ => unreachable!(),
            }
        }
    }

    let mut world = system_test! {
        test_system.build();
        _: TestArch = (
            Simple1OptionalNoDepNoInit(5),
            @(TestDiscrim1(7), IsoNoInit(11)),
            @(TestDiscrim1(31), IsoNoInit(41)),
        );
        _: TestArch = (
            @(TestDiscrim1(13), IsoNoInit(17)),
            @(TestDiscrim1(31), IsoNoInit(43)),
        );
        _: TestArch = (
            @(TestDiscrim1(7), IsoNoInit(19)),
            @(TestDiscrim1(13), IsoNoInit(23)),
        );
    };

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
fn test_entity_iter_full_mut() {
    #[system(dynec_as(crate))]
    fn test_system(
        iter: system::EntityIterator<TestArch>,
        comp1_acc: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
        mut iso1_acc: system::WriteIsotopeFull<TestArch, IsoNoInit>,
    ) {
        let [mut iso1_acc_0, mut iso1_acc_1] =
            iso1_acc.split_isotopes([TestDiscrim1(7), TestDiscrim1(13)]);

        for (entity, (comp1, iso10, iso11)) in iter.entities_with((
            system::Try(&comp1_acc),
            system::Try(&mut iso1_acc_0),
            system::Try(&mut iso1_acc_1),
        )) {
            match entity.id().to_primitive() {
                1 => {
                    assert_eq!(comp1, Some(&Simple1OptionalNoDepNoInit(5)));
                    assert_eq!(iso10, Some(&mut IsoNoInit(11)));
                    assert_eq!(iso11, None);
                }
                2 => {
                    assert_eq!(comp1, None);
                    assert_eq!(iso10, None);
                    assert_eq!(iso11, Some(&mut IsoNoInit(17)));
                }
                3 => {
                    assert_eq!(comp1, None);
                    assert_eq!(iso10, Some(&mut IsoNoInit(19)));
                    assert_eq!(iso11, Some(&mut IsoNoInit(23)));
                }
                _ => unreachable!(),
            }
        }
    }

    let mut world = system_test! {
        test_system.build();
        _: TestArch = (
            Simple1OptionalNoDepNoInit(5),
            @(TestDiscrim1(7), IsoNoInit(11)),
        );
        _: TestArch = (
            @(TestDiscrim1(13), IsoNoInit(17)),
        );
        _: TestArch = (
            @(TestDiscrim1(7), IsoNoInit(19)),
            @(TestDiscrim1(13), IsoNoInit(23)),
        );
    };

    world.execute(&tracer::Log(log::Level::Trace));
}

// Test that there is no access conflict when creating, deleting and iterating the same archetype.
#[test]
fn test_entity_create_and_delete() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_creator: system::EntityCreator<TestArch>,
        _entity_deleter: system::EntityDeleter<TestArch>,
        entity_iter: system::EntityIterator<TestArch>,
    ) {
        let entity = entity_creator
            .create(crate::comps![ @(crate) TestArch => Simple1OptionalNoDepNoInit(1) ]);
        for v in entity_iter.entities() {
            assert_ne!(entity.id(), v.id());
        }
    }

    #[system(dynec_as(crate))]
    fn dummy_reader_system(_: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>) {}

    let mut world = system_test! {
        test_system.build(), dummy_reader_system.build();
    };
    world.execute(&tracer::Log(log::Level::Trace));
}
