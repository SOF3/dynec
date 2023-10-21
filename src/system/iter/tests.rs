//! Tests EntityIterator.

use rayon::prelude::ParallelIterator;

use crate::entity::{Raw as _, Ref};
use crate::test_util::*;
use crate::{system, system_test, system_test_exported, tracer, world};

macro_rules! test_partial_single_system {
    ($test_name:ident $iter_method:ident) => {
        #[test]
        fn $test_name() {
            #[system(dynec_as(crate))]
            fn test_system(
                iter: system::EntityIterator<TestArch>,
                simple_acc: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
                #[dynec(isotope(discrim = [TestDiscrim1(7), TestDiscrim1(13)]))]
                mut double_iso_acc: system::WriteIsotopePartial<TestArch, IsoNoInit, [TestDiscrim1; 2]>,
                #[dynec(isotope(discrim = [TestDiscrim1(31)]))]
                mut single_iso_acc: system::ReadIsotopePartial<TestArch, IsoNoInit, [TestDiscrim1; 1]>,
            ) {
                let [mut double_iso_acc_0, mut double_iso_acc_1] =
                    double_iso_acc.split_mut([0, 1]);
                let [single_iso_acc_0] = single_iso_acc.split([0]);

                let iter_collected: Vec<_> = iter
                    .$iter_method((
                        system::Try(&simple_acc),
                        system::Try(&mut double_iso_acc_0),
                        system::Try(&mut double_iso_acc_1),
                        system::Try(&single_iso_acc_0),
                    ))
                    .collect();
                for (entity, (simple, double0, double1, single)) in iter_collected {
                    match entity.id().to_primitive() {
                        1 => {
                            assert_eq!(simple, Some(&Simple1OptionalNoDepNoInit(5)));
                            assert_eq!(double0, Some(&mut IsoNoInit(11)));
                            assert_eq!(double1, None);
                            assert_eq!(single, Some(&IsoNoInit(41)));
                        }
                        2 => {
                            assert_eq!(simple, None);
                            assert_eq!(double0, None);
                            assert_eq!(double1, Some(&mut IsoNoInit(17)));
                            assert_eq!(single, Some(&IsoNoInit(43)));
                        }
                        3 => {
                            assert_eq!(simple, None);
                            assert_eq!(double0, Some(&mut IsoNoInit(19)));
                            assert_eq!(double1, None);
                            assert_eq!(single, None);
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
                );
            };

            world.execute(&tracer::Log(log::Level::Trace));
        }
    };
}

test_partial_single_system!(test_partial_single_serial entities_with);
test_partial_single_system!(test_partial_single_chunked par_entities_with);

#[test]
fn test_entity_iter_partial_chunked_mut() {
    #[system(dynec_as(crate))]
    fn test_system(
        iter: system::EntityIterator<TestArch>,
        simple_acc: system::ReadSimple<TestArch, Simple5RequiredNoInit>,
        #[dynec(isotope(discrim = [TestDiscrim2(7), TestDiscrim2(13)]))]
        mut double_iso_acc: system::WriteIsotopePartial<
            TestArch,
            IsoWithInit,
            [TestDiscrim2; 2],
        >,
        #[dynec(isotope(discrim = [TestDiscrim2(31)]))]
        mut single_iso_acc: system::ReadIsotopePartial<
            TestArch,
            IsoWithInit,
            [TestDiscrim2; 1],
        >,
    ) {
        let [mut double_iso_acc_0, double_iso_acc_1] = double_iso_acc.split_mut([0, 1]);
        let [single_iso_acc_0] = single_iso_acc.split([0]);

        for (chunk_enumerate, (chunk, (simple, double0, double1, single))) in iter
            .chunks_with((&simple_acc, &mut double_iso_acc_0, &double_iso_acc_1, &single_iso_acc_0))
            .enumerate()
        {
            match chunk_enumerate {
                0 => {
                    assert_eq!(chunk.start.get(), 1);
                    assert_eq!(chunk.end.get(), 3);

                    assert_eq!(simple[0], Simple5RequiredNoInit(5));
                    assert_eq!(double0[0], IsoWithInit(11));
                    assert_eq!(double1[0], IsoWithInit(73));
                    assert_eq!(single[0], IsoWithInit(41));

                    assert_eq!(simple[1], Simple5RequiredNoInit(47));
                    assert_eq!(double0[1], IsoWithInit(73));
                    assert_eq!(double1[1], IsoWithInit(17));
                    assert_eq!(single[1], IsoWithInit(43));
                }
                1 => {
                    assert_eq!(chunk.start.get(), 4);
                    assert_eq!(chunk.end.get(), 5);

                    assert_eq!(simple[0], Simple5RequiredNoInit(53));
                    assert_eq!(double0[0], IsoWithInit(19));
                    assert_eq!(double1[0], IsoWithInit(23));
                    assert_eq!(single[0], IsoWithInit(73));
                }
                _ => unreachable!(),
            }
        }
    }

    let (mut world, (hole,)) = system_test_exported! {
        test_system.build();
        TestArch = (
            Simple5RequiredNoInit(5),
            @(TestDiscrim2(7), IsoWithInit(11)),
            @(TestDiscrim2(31), IsoWithInit(41)),
        );
        TestArch = (
            Simple5RequiredNoInit(47),
            @(TestDiscrim2(13), IsoWithInit(17)),
            @(TestDiscrim2(31), IsoWithInit(43)),
        );
        let hole: TestArch = (
            Simple5RequiredNoInit(404),
        );
        TestArch = (
            Simple5RequiredNoInit(53),
            @(TestDiscrim2(7), IsoWithInit(19)),
            @(TestDiscrim2(13), IsoWithInit(23)),
        );
    };

    let delete_result = world.delete(hole);
    assert_eq!(delete_result, world::DeleteResult::Deleted);

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
            iso1_acc.split_mut([TestDiscrim1(7), TestDiscrim1(13)]);

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
