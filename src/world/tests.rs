#![allow(clippy::ptr_arg)]

use crate::entity::{deletion, generation, Ref};
use crate::test_util::*;
use crate::{global, system, system_test, tracer, world, Entity};

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
#[should_panic = "The component TestArch/Simple2OptionalDepends1 cannot be used because it is not \
                  used in any systems"]
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

    world.components.get_simple_storage::<TestArch, Simple2OptionalDepends1>();
    // panic here
}

#[test]
#[should_panic = "Cannot create an entity of type `dynec::test_util::TestArch` without explicitly \
                  passing a component of type `dynec::test_util::Simple5RequiredNoInit`"]
fn test_dependencies_missing_required_simple() {
    let mut world = system_test!(common_test_system.build(););
    world.create::<TestArch>(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(1)]);
}

#[test]
#[should_panic = "Cannot create an entity of type `dynec::test_util::TestArch` without explicitly \
                  passing a component of type `dynec::test_util::Simple2OptionalDepends1`, or \
                  `dynec::test_util::Simple1OptionalNoDepNoInit` to invoke its auto-initializer"]
fn test_dependencies_missing_required_dep() {
    let mut world = system_test!(common_test_system.build(););
    world.create::<TestArch>(crate::comps![@(crate) TestArch => Simple5RequiredNoInit(1)]);
}

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
#[should_panic = "Global type dynec::world::tests::test_global_uninit::Uninit does not have an \
                  initial impl and was not provided manually"]
fn test_global_uninit() {
    #[global(dynec_as(crate))]
    struct Uninit;

    #[system(dynec_as(crate))]
    fn test_system(#[dynec(global)] _: &Uninit) {}

    let _world = system_test!(test_system.build(););
}

#[test]
fn test_simple_fetch() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut comp5: system::WriteSimple<TestArch, Simple5RequiredNoInit>,
        #[dynec(global)] initials: &InitialEntities,
    ) {
        let ent = initials.strong.as_ref().expect("initials.strong is None");

        let comp = comp5.get_mut(ent);
        assert_eq!(comp.0, 7);
        comp.0 += 13;
    }

    let mut world = system_test!(test_system.build(););

    let ent = world.create(crate::comps![@(crate) TestArch => Simple5RequiredNoInit(7)]);
    world.get_global::<InitialEntities>().strong = Some(ent.clone());

    world.execute(&tracer::Log(log::Level::Trace));

    let comp =
        world.components.get_simple_storage::<TestArch, Simple5RequiredNoInit>().try_get(ent);
    assert_eq!(comp, Some(&Simple5RequiredNoInit(20)));
}

fn isotope_discrim_read_test_system(
    mut iso1: system::AccessIsotope<
        TestArch,
        IsoNoInit,
        impl system::access::StorageMap<TestArch, IsoNoInit, Key = TestDiscrim1>,
    >,
    mut iso2: system::AccessIsotope<
        TestArch,
        IsoWithInit,
        impl system::access::StorageMap<TestArch, IsoWithInit, Key = TestDiscrim2>,
    >,
    initials: &InitialEntities,
) {
    let ent = initials.strong.as_ref().expect("initials.strong is None");

    {
        let iso = iso1.try_get(ent, TestDiscrim1(11));
        assert_eq!(iso, Some(&IsoNoInit(3)));
    }

    // should not panic on nonexistent storages
    {
        let iso = iso1.try_get(ent, TestDiscrim1(17));
        assert_eq!(iso, None);
    }

    // should return default value for autoinit isotopes
    {
        let iso = iso2.try_get(ent, TestDiscrim2(71));
        assert_eq!(iso, Some(&IsoWithInit(73)));
    }

    let map = iso1.get_all(ent);
    let mut map_vec: Vec<(TestDiscrim1, &IsoNoInit)> = map.collect();
    map_vec.sort_by_key(|(TestDiscrim1(discrim), _)| *discrim);
    assert_eq!(map_vec, vec![(TestDiscrim1(11), &IsoNoInit(3)), (TestDiscrim1(13), &IsoNoInit(5))]);
}

fn isotope_discrim_test_world(system: impl system::Sendable + 'static) -> world::World {
    let mut world = system_test!(system;);

    let ent = world.create(crate::comps![@(crate) TestArch =>
        @(TestDiscrim1(11), IsoNoInit(3)),
        @(TestDiscrim1(13), IsoNoInit(5)),
    ]);
    world.get_global::<InitialEntities>().strong = Some(ent);

    world
}

#[test]
fn test_full_isotope_discrim_write() {
    #[system(dynec_as(crate))]
    fn test_sys(
        iso1: system::WriteIsotopeFull<TestArch, IsoNoInit>,
        iso2: system::WriteIsotopeFull<TestArch, IsoWithInit>,
        #[dynec(global)] initials: &InitialEntities,
    ) {
        isotope_discrim_read_test_system(iso1, iso2, initials);
    }

    let mut world = isotope_discrim_test_world(test_sys.build());

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
fn test_full_isotope_discrim_read() {
    #[system(dynec_as(crate))]
    fn test_system(
        iso1: system::ReadIsotopeFull<TestArch, IsoNoInit>,
        iso2: system::ReadIsotopeFull<TestArch, IsoWithInit>,
        #[dynec(global)] initials: &InitialEntities,
    ) {
        isotope_discrim_read_test_system(iso1, iso2, initials)
    }

    let mut world = isotope_discrim_test_world(test_system.build());
    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
fn test_partial_isotope_discrim_write() {
    partial_isotope_discrim_write(
        vec![TestDiscrim1(7), TestDiscrim1(11), TestDiscrim1(17), TestDiscrim1(19)],
        vec![
            (0, Some(IsoNoInit(2)), Some(None)),
            (1, Some(IsoNoInit(3)), Some(Some(IsoNoInit(23)))),
            (2, None, None),
            (3, None, Some(Some(IsoNoInit(29)))),
        ],
        vec![(TestDiscrim1(11), IsoNoInit(23)), (TestDiscrim1(19), IsoNoInit(29))],
    );
}

#[test]
fn test_partial_isotope_discrim_read() {
    partial_isotope_discrim_read(
        vec![TestDiscrim1(11), TestDiscrim1(17)],
        vec![(0, Some(IsoNoInit(3))), (1, None)],
        vec![(TestDiscrim1(11), IsoNoInit(3))],
    );
}

#[test]
#[should_panic = "The index 42 is not available in the isotope request for \
                  dynec::test_util::TestArch/dynec::test_util::IsoNoInit"]
fn test_partial_isotope_discrim_read_panic() {
    partial_isotope_discrim_read(vec![TestDiscrim1(11)], vec![(42, None)], vec![]);
}

fn partial_isotope_discrim_read(
    req_discrims: Vec<TestDiscrim1>,
    single_expects: Vec<(usize, Option<IsoNoInit>)>,
    expect_all: Vec<(TestDiscrim1, IsoNoInit)>,
) {
    #[system(dynec_as(crate))]
    fn test_system(
        #[dynec(param)] _req_discrims: &Vec<TestDiscrim1>,
        #[dynec(param)] single_expects: &Vec<(usize, Option<IsoNoInit>)>,
        #[dynec(param)] expect_all: &Vec<(TestDiscrim1, IsoNoInit)>,
        #[dynec(isotope(discrim = _req_discrims))] mut iso1: system::ReadIsotopePartial<
            TestArch,
            IsoNoInit,
        >,
        #[dynec(global)] initials: &InitialEntities,
    ) {
        let ent = initials.strong.as_ref().expect("initials.strong is None");

        for (discrim, expect) in single_expects {
            let iso = iso1.try_get(ent, *discrim);
            assert_eq!(iso, expect.as_ref());
        }

        // should only include requested discriminants
        let map = iso1.get_all(ent);
        let mut map_vec: Vec<(TestDiscrim1, &IsoNoInit)> = map.collect();
        map_vec.sort_by_key(|(TestDiscrim1(discrim), _)| *discrim);
        let expect_all =
            expect_all.iter().map(|(discrim, iso)| (*discrim, iso)).collect::<Vec<_>>();
        assert_eq!(map_vec, expect_all);
    }

    let mut world = system_test!(
        test_system.build(req_discrims, single_expects, expect_all);
    );

    let ent = world.create(crate::comps![@(crate) TestArch =>
        @(TestDiscrim1(11), IsoNoInit(3)),
        @(TestDiscrim1(13), IsoNoInit(5)),
    ]);
    world.get_global::<InitialEntities>().strong = Some(ent);

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
#[should_panic = "The index 42 is not available in the isotope request for \
                  dynec::test_util::TestArch/dynec::test_util::IsoNoInit"]
fn test_partial_isotope_discrim_write_panic() {
    partial_isotope_discrim_write(vec![TestDiscrim1(11)], vec![(42, None, None)], vec![]);
}

type SingleExpectUpdate = (usize, Option<IsoNoInit>, Option<Option<IsoNoInit>>);

fn partial_isotope_discrim_write(
    req_discrims: Vec<TestDiscrim1>,
    single_expect_updates: Vec<SingleExpectUpdate>,
    expect_all: Vec<(TestDiscrim1, IsoNoInit)>,
) {
    #[system(dynec_as(crate))]
    fn test_system(
        #[dynec(param)] _req_discrims: &Vec<TestDiscrim1>,
        #[dynec(param)] single_expect_updates: &mut Vec<SingleExpectUpdate>,
        #[dynec(param)] expect_all: &Vec<(TestDiscrim1, IsoNoInit)>,
        #[dynec(isotope(discrim = _req_discrims))] mut iso1: system::WriteIsotopePartial<
            TestArch,
            IsoNoInit,
        >,
        #[dynec(global)] initials: &InitialEntities,
    ) {
        let ent = initials.strong.as_ref().expect("initials.strong is None");

        for (discrim, mut expect, update) in single_expect_updates.drain(..) {
            let iso = iso1.try_get_mut(ent, discrim);
            assert_eq!(iso, expect.as_mut());
            if let Some(update) = update {
                iso1.set(ent, discrim, update);
            }
        }

        // should only include requested discriminants
        let map = iso1.get_all(ent);
        let map_vec: Vec<(TestDiscrim1, &IsoNoInit)> = map.collect();
        let expect_all =
            expect_all.iter().map(|(discrim, iso)| (*discrim, iso)).collect::<Vec<_>>();
        assert_eq!(map_vec, expect_all);
    }

    let mut world =
        system_test!(test_system.build(req_discrims, single_expect_updates, expect_all););

    let ent = world.create(crate::comps![@(crate) TestArch =>
        @(TestDiscrim1(7), IsoNoInit(2)),
        @(TestDiscrim1(11), IsoNoInit(3)),
        @(TestDiscrim1(13), IsoNoInit(5)),
    ]);
    world.get_global::<InitialEntities>().strong = Some(ent);

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
fn test_offline_create() {
    #[global(dynec_as(crate), initial = Step::Create)]
    enum Step {
        Create,
        Access,
    }

    #[derive(Debug, PartialEq, Eq, Hash)]
    struct LatePartition;

    #[system(dynec_as(crate), before(LatePartition))]
    fn entity_creator_system(
        mut entity_creator: system::EntityCreator<TestArch>,
        #[dynec(global(maybe_uninit(TestArch)))] initials: &mut InitialEntities,
        #[dynec(global)] step: &Step,
    ) {
        match step {
            Step::Create => {
                initials.strong = Some(
                    entity_creator
                        .create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(5)]),
                );
            }
            Step::Access => {}
        }
    }

    #[system(dynec_as(crate))]
    fn comp_access_system(
        comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
        #[dynec(global)] initials: &InitialEntities,
        #[dynec(global)] step: &Step,
    ) {
        match step {
            Step::Create => {
                assert!(initials.strong.is_none());
            }
            Step::Access => {
                let ent = initials.strong.as_ref().expect("initials.strong should have been set");
                comp1.try_get(ent).expect("initials.strong should have been initialized");
            }
        }
    }

    #[system(dynec_as(crate), after(LatePartition))]
    fn late_comp_access_system(
        // component storage does not require maybe_uninit unless the component has something like `Option<Box<Self>>`
        comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
        #[dynec(global(maybe_uninit(TestArch)))] initials: &InitialEntities,
        #[dynec(global)] step: &Step,
    ) {
        match step {
            Step::Create => {
                let ent = initials.strong.as_ref().expect("initials.strong should have been set");
                assert!(comp1.try_get(ent).is_none(), "entity should be in pre-initialize state");
            }
            Step::Access => {
                let ent = initials.strong.as_ref().expect("initials.strong should have been set");
                comp1.try_get(ent).expect("initials.strong should have been initialized");
            }
        }
    }

    let mut world = system_test!(comp_access_system.build(), late_comp_access_system.build(), entity_creator_system.build(););

    world.execute(&tracer::Log(log::Level::Trace));
    *world.get_global::<Step>() = Step::Access;
    world.execute(&tracer::Log(log::Level::Trace));

    let ent = {
        let initials = world.get_global::<InitialEntities>();
        let ent = initials.strong.as_ref().expect("initials.strong missing");
        ent.clone()
    };
    let comp1 =
        world.components.get_simple_storage::<TestArch, Simple1OptionalNoDepNoInit>().try_get(&ent);
    assert_eq!(comp1, Some(&Simple1OptionalNoDepNoInit(5)));
}

#[test]
#[should_panic = "Scheduled systems have a cyclic dependency: "]
fn test_offline_create_conflict() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_creator: system::EntityCreator<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    ) {
        initials.strong = Some(
            entity_creator
                .create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(5)]),
        );
    }

    let mut world = system_test!(test_system.build(););

    world.execute(&tracer::Log(log::Level::Trace));

    let ent = {
        let initials = world.get_global::<InitialEntities>();
        let ent = initials.strong.as_ref().expect("initials.strong missing");
        ent.clone()
    };
    let comp1 =
        world.components.get_simple_storage::<TestArch, Simple1OptionalNoDepNoInit>().try_get(&ent);
    assert_eq!(comp1, Some(&Simple1OptionalNoDepNoInit(5)));
}

#[test]
fn test_offline_delete() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    ) {
        entity_deleter.queue(initials.strong.take().expect("initials.strong missing"));
    }

    let mut world = system_test!(test_system.build(););
    let ent = world.create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(7)]);
    let weak = ent.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().strong = Some(ent);

    world.execute(&tracer::Log(log::Level::Trace));

    let comp1 = world
        .components
        .get_simple_storage::<TestArch, Simple1OptionalNoDepNoInit>()
        .try_get(&weak);
    assert_eq!(comp1, None);
}

#[test]
#[cfg_attr(
    any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ),
    should_panic = "Detected dangling strong reference to entity dynec::test_util::TestArch#1 in \
                    system dynec::world::tests::test_system. All strong references to an entity \
                    must be dropped before queuing for deletion and removing all finalizers."
)]
fn test_offline_delete_send_system_leak() {
    #[system(dynec_as(crate))]
    fn test_system(
        #[dynec(local(initial = None, entity))] entity: &mut Option<Entity<TestArch>>,
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    ) {
        if let Some(ent) = initials.strong.take() {
            *entity = Some(ent);
        }

        if let Some(ent) = entity {
            entity_deleter.queue(&*ent);
        }
    }

    let mut builder = world::Builder::new(0);
    builder.schedule(Box::new(test_system.build()));

    let mut world = builder.build();

    let ent = world.create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(7)]);
    let weak = ent.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().strong = Some(ent);

    world.execute(&tracer::Log(log::Level::Trace));

    let comp1 = world
        .components
        .get_simple_storage::<TestArch, Simple1OptionalNoDepNoInit>()
        .try_get(&weak);
    assert_eq!(comp1, None);
}

#[test]
#[cfg_attr(
    any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ),
    should_panic = "Detected dangling strong reference to entity dynec::test_util::TestArch#1 in \
                    system dynec::world::tests::test_system. All strong references to an entity \
                    must be dropped before queuing for deletion and removing all finalizers."
)]
fn test_offline_delete_unsend_system_leak() {
    #[system(dynec_as(crate), thread_local)]
    fn test_system(
        #[dynec(local(initial = None, entity))] entity: &mut Option<Entity<TestArch>>,
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    ) {
        if let Some(ent) = initials.strong.take() {
            *entity = Some(ent);
        }

        if let Some(ent) = entity {
            entity_deleter.queue(&*ent);
        }
    }

    let mut builder = world::Builder::new(0);
    builder.schedule_thread_unsafe(Box::new(test_system.build()));

    let mut world = builder.build();

    let ent = world.create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(7)]);
    let weak = ent.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().strong = Some(ent);

    world.execute(&tracer::Log(log::Level::Trace));

    let comp1 = world
        .components
        .get_simple_storage::<TestArch, Simple1OptionalNoDepNoInit>()
        .try_get(&weak);
    assert_eq!(comp1, None);
}

#[test]
#[cfg_attr(
    any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ),
    should_panic = "Detected dangling strong reference to entity dynec::test_util::TestArch#1 in \
                    global state dynec::test_util::InitialEntities. All strong references to an \
                    entity must be dropped before queuing for deletion and removing all \
                    finalizers."
)]
fn test_offline_delete_sync_global_leak() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    ) {
        entity_deleter.queue(initials.strong.as_ref().expect("initials.strong missing"));
    }

    let mut world = system_test!(test_system.build(););
    let ent = world.create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(7)]);
    world.get_global::<InitialEntities>().strong = Some(ent);

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
#[cfg_attr(
    any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ),
    should_panic = "Detected dangling strong reference to entity dynec::test_util::TestArch#1 in \
                    global state dynec::test_util::InitialEntities. All strong references to an \
                    entity must be dropped before queuing for deletion and removing all \
                    finalizers."
)]
fn test_offline_delete_unsync_global_leak() {
    #[system(dynec_as(crate), thread_local)]
    fn test_system(
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global(thread_local))] initials: &mut InitialEntities,
        _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    ) {
        entity_deleter.queue(initials.strong.as_ref().expect("initials.strong missing"));
    }

    let mut builder = world::Builder::new(0);
    builder.schedule_thread_unsafe(Box::new(test_system.build()));

    let mut world = builder.build();

    let ent = world.create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(7)]);
    world.get_global_unsync::<InitialEntities>().strong = Some(ent);

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
#[cfg_attr(
    any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ),
    should_panic = "Detected dangling strong reference to entity dynec::test_util::TestArch#1 in \
                    dynec::test_util::TestArch / dynec::test_util::StrongRefSimple. All strong \
                    references to an entity must be dropped before queuing for deletion and \
                    removing all finalizers."
)]
fn test_offline_delete_simple_leak() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _srs: system::ReadSimple<TestArch, StrongRefSimple>,
    ) {
        let entity = initials.weak.as_ref().expect("initials.strong missing");
        entity_deleter.queue(entity);
    }

    let mut builder = world::Builder::new(0);
    builder.schedule(Box::new(test_system.build()));

    let mut world = builder.build();

    let ent = world.create(crate::comps![@(crate) TestArch =>]);
    let weak = ent.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().weak = Some(weak);

    world.create(crate::comps![@(crate) TestArch => StrongRefSimple(ent)]);

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
#[cfg_attr(
    any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ),
    should_panic = "Detected dangling strong reference to entity dynec::test_util::TestArch#1 in \
                    dynec::test_util::TestArch / dynec::test_util::StrongRefIsotope # \
                    TestDiscrim1(29). All strong references to an entity must be dropped before \
                    queuing for deletion and removing all finalizers."
)]
fn test_offline_delete_isotope_leak() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _sri: system::ReadIsotopeFull<TestArch, StrongRefIsotope>,
    ) {
        let entity = initials.weak.as_ref().expect("initials.strong missing");
        entity_deleter.queue(entity);
    }

    let mut builder = world::Builder::new(0);
    builder.schedule(Box::new(test_system.build()));

    let mut world = builder.build();

    let ent = world.create(crate::comps![@(crate) TestArch =>]);
    let weak = ent.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().weak = Some(weak);

    world.create(crate::comps![@(crate) TestArch => @(TestDiscrim1(29), StrongRefIsotope(ent))]);

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
fn test_offline_finalizer_delete() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_deleter: system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        deletion_flags: system::ReadSimple<TestArch, deletion::Flag>,
        mut comp_final: system::WriteSimple<TestArch, Simple7WithFinalizerNoinit>,
        _comp1: system::ReadSimple<TestArch, Simple1OptionalNoDepNoInit>,
    ) {
        let ent = initials.strong.as_ref().expect("initials.strong missing");
        if deletion_flags.try_get(ent).is_some() {
            comp_final.set(ent, None);
            initials.strong = None;
        } else {
            entity_deleter.queue(ent);
        }
    }

    let mut world = system_test!(test_system.build(););

    for _ in 0..3 {
        let ent = world.create(crate::comps![@(crate) TestArch => Simple1OptionalNoDepNoInit(13), Simple7WithFinalizerNoinit]);
        let weak = ent.weak(world.get_global::<generation::StoreMap>());
        world.get_global::<InitialEntities>().strong = Some(ent);

        // first iteration
        world.execute(&tracer::Log(log::Level::Trace));

        let comp1 = world
            .components
            .get_simple_storage::<TestArch, Simple1OptionalNoDepNoInit>()
            .try_get(&weak);
        assert_eq!(comp1, Some(&Simple1OptionalNoDepNoInit(13)));

        // second iteration
        world.execute(&tracer::Log(log::Level::Trace));

        let comp1 = world
            .components
            .get_simple_storage::<TestArch, Simple1OptionalNoDepNoInit>()
            .try_get(&weak);
        assert_eq!(comp1, None);
    }
}

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
            comp1_acc.try_access(),
            iso1_acc_0.try_access_mut(),
            iso1_acc_1.try_access_mut(),
            iso1_acc_31.try_access(),
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
            comp1_acc.try_access(),
            iso1_acc_0.try_access_mut(),
            iso1_acc_1.try_access_mut(),
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
