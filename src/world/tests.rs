#![allow(clippy::ptr_arg)]

use super::tracer;
use crate::entity::{deletion, generation, Ref};
use crate::{
    comp, global, system, system_test, world, Entity, TestArch, TestDiscrim1, TestDiscrim2,
};

// Test component summary:
// Comp1: optional, depends []
// Comp2: optional, depends on Comp2
// Comp3: optional, depends on Comp1 and Comp2
// Comp4: optional, depends on Comp1 and Comp2
// Comp5: required, no init
// Comp6: required, depends []

#[comp(dynec_as(crate), of = TestArch)]
#[derive(Debug, PartialEq)]
struct Comp1(i32);

#[comp(dynec_as(crate), of = TestArch, init = init_comp2/1)]
#[derive(Debug)]
struct Comp2(i32);
fn init_comp2(c1: &Comp1) -> Comp2 { Comp2(c1.0 + 2) }

#[comp(
    dynec_as(crate),
    of = TestArch,
    init = |c1: &Comp1, c2: &Comp2| Comp3(c1.0 * 3, c2.0 * 5),
)]
#[derive(Debug)]
struct Comp3(i32, i32);

#[comp(
    dynec_as(crate),
    of = TestArch,
    init = |c1: &Comp1, c2: &Comp2| Comp4(c1.0 * 7, c2.0 * 8),
)]
#[derive(Debug, PartialEq)]
struct Comp4(i32, i32);

#[comp(dynec_as(crate), of = TestArch, required)]
#[derive(Debug, PartialEq)]
struct Comp5(i32);
#[comp(dynec_as(crate), of = TestArch, required, init = || Comp6(9))]
#[derive(Debug)]
struct Comp6(i32);

#[comp(dynec_as(crate), of = TestArch, finalizer)]
struct CompFinal;

/// Does not have auto init
#[comp(dynec_as(crate), of = TestArch, isotope = TestDiscrim1)]
#[derive(Debug, Clone, PartialEq)]
struct Iso1(i32);
/// Has auto init
#[comp(dynec_as(crate), of = TestArch, isotope = TestDiscrim2, init = || Iso2(-1))]
#[derive(Debug, Clone, PartialEq)]
struct Iso2(i32);

#[comp(dynec_as(crate), of = TestArch)]
struct StrongRefSimple(#[entity] Entity<TestArch>);

#[comp(dynec_as(crate), of = TestArch, isotope = TestDiscrim1)]
struct StrongRefIsotope(#[entity] Entity<TestArch>);

#[global(dynec_as(crate), initial)]
#[derive(Default)]
struct Aggregator {
    comp30_sum:     i32,
    comp41_product: i32,
}

#[global(dynec_as(crate), initial)]
#[derive(Default)]
struct InitialEntities {
    #[entity]
    ent1: Option<Entity<TestArch>>,
}

#[system(dynec_as(crate))]
fn test_system(
    _comp3: impl system::ReadSimple<TestArch, Comp3>,
    _comp4: impl system::WriteSimple<TestArch, Comp4>,
    _comp5: impl system::ReadSimple<TestArch, Comp5>,
    _comp6: impl system::ReadSimple<TestArch, Comp6>,
    #[dynec(isotope(discrim = [TestDiscrim1(11), TestDiscrim1(17)]))] _iso1: impl system::ReadIsotope<
        TestArch,
        Iso1,
        usize,
    >,
    #[dynec(global)] _aggregator: &mut Aggregator,
    #[dynec(global)] _initials: &InitialEntities,
) {
}

#[test]
#[should_panic = "The component dynec::world::tests::Comp2 cannot be retrieved because it is not \
                  used in any systems"]
fn test_dependencies_successful() {
    let mut world = system_test!(test_system.build(););
    let entity = world.create::<TestArch>(crate::comps![ @(crate) TestArch =>
        Comp1(1), Comp5(1),
        @(TestDiscrim1(11), Iso1(1)),
        @(TestDiscrim1(13), Iso1(2)),
        @(TestDiscrim1(17), Iso1(3)),
    ]);

    match world.get_simple::<TestArch, Comp4, _>(&entity) {
        Some(&mut Comp4(c40, c41)) => {
            assert_eq!(c40, 7);
            assert_eq!(c41, (1 + 2) * 8);
        }
        None => panic!("Comp4 is used in system_with_comp3_comp4_comp5"),
    }

    world.get_simple::<TestArch, Comp2, _>(&entity); // panic here
}

#[test]
#[should_panic = "Cannot create an entity of type `dynec::test_util::TestArch` without explicitly \
                  passing a component of type `dynec::world::tests::Comp5`"]
fn test_dependencies_missing_required_simple() {
    let mut world = system_test!(test_system.build(););
    world.create::<TestArch>(crate::comps![@(crate) TestArch => Comp1(1)]);
}

#[test]
#[should_panic = "Cannot create an entity of type `dynec::test_util::TestArch` without explicitly \
                  passing a component of type `dynec::world::tests::Comp1`, which is required for \
                  `dynec::world::tests::Comp2`"]
fn test_dependencies_missing_required_dep() {
    let mut world = system_test!(test_system.build(););
    world.create::<TestArch>(crate::comps![@(crate) TestArch => Comp5(1)]);
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
        mut comp5: impl system::WriteSimple<TestArch, Comp5>,
        #[dynec(global)] initials: &InitialEntities,
    ) {
        let ent1 = initials.ent1.as_ref().expect("ent1 is None");

        let comp = comp5.get_mut(ent1);
        assert_eq!(comp.0, 7);
        comp.0 += 13;
    }

    let mut world = system_test!(test_system.build(););

    let ent1 = world.create(crate::comps![@(crate) TestArch => Comp5(7)]);
    world.get_global::<InitialEntities>().ent1 = Some(ent1.clone());

    world.execute(&tracer::Log(log::Level::Trace));

    let comp = world.get_simple::<TestArch, Comp5, _>(ent1);
    assert_eq!(comp, Some(&mut Comp5(20)));
}

#[test]
fn test_full_isotope_discrim_read() {
    #[system(dynec_as(crate))]
    fn test_system(
        iso1: impl system::ReadIsotope<TestArch, Iso1>,
        iso2: impl system::ReadIsotope<TestArch, Iso2>,
        #[dynec(global)] initials: &InitialEntities,
    ) {
        let ent1 = initials.ent1.as_ref().expect("ent1 is None");

        {
            let iso = iso1.try_get(ent1, TestDiscrim1(11));
            assert_eq!(iso.as_deref(), Some(&Iso1(3)));
        }

        // should not panic on nonexistent storages
        {
            let iso = iso1.try_get(ent1, TestDiscrim1(17));
            assert_eq!(iso.as_deref(), None);
        }

        // should return default value for autoinit isotopes
        {
            let iso = iso2.try_get(ent1, TestDiscrim2(31));
            assert_eq!(iso.as_deref(), Some(&Iso2(-1)));
        }

        let map = iso1.get_all(ent1);
        let mut map_vec: Vec<(TestDiscrim1, &Iso1)> = map.collect();
        map_vec.sort_by_key(|(TestDiscrim1(discrim), _)| *discrim);
        assert_eq!(map_vec, vec![(TestDiscrim1(11), &Iso1(3)), (TestDiscrim1(13), &Iso1(5))]);
    }

    let mut world = system_test!(test_system.build(););

    let ent1 = world.create(crate::comps![@(crate) TestArch =>
        @(TestDiscrim1(11), Iso1(3)),
        @(TestDiscrim1(13), Iso1(5)),
    ]);
    world.get_global::<InitialEntities>().ent1 = Some(ent1);

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
fn test_partial_isotope_discrim_read() {
    partial_isotope_discrim_read(
        vec![TestDiscrim1(11), TestDiscrim1(17)],
        vec![(0, Some(Iso1(3))), (1, None)],
        vec![(TestDiscrim1(11), Iso1(3))],
    );
}

#[test]
#[should_panic = "The index 42 is not available in the isotope request for \
                  dynec::test_util::TestArch/dynec::world::tests::Iso1"]
fn test_partial_isotope_discrim_read_panic() {
    partial_isotope_discrim_read(vec![TestDiscrim1(11)], vec![(42, None)], vec![]);
}

fn partial_isotope_discrim_read(
    req_discrims: Vec<TestDiscrim1>,
    single_expects: Vec<(usize, Option<Iso1>)>,
    expect_all: Vec<(TestDiscrim1, Iso1)>,
) {
    #[system(dynec_as(crate))]
    fn test_system(
        #[dynec(param)] _req_discrims: &Vec<TestDiscrim1>,
        #[dynec(param)] single_expects: &Vec<(usize, Option<Iso1>)>,
        #[dynec(param)] expect_all: &Vec<(TestDiscrim1, Iso1)>,
        #[dynec(isotope(discrim = _req_discrims))] iso1: impl system::ReadIsotope<TestArch, Iso1, usize>,
        #[dynec(global)] initials: &InitialEntities,
    ) {
        let ent1 = initials.ent1.as_ref().expect("ent1 is None");

        for (discrim, expect) in single_expects {
            let iso = iso1.try_get(ent1, *discrim);
            assert_eq!(iso.as_deref(), expect.as_ref());
        }

        // should only include requested discriminants
        let map = iso1.get_all(ent1);
        let mut map_vec: Vec<(TestDiscrim1, &Iso1)> = map.collect();
        map_vec.sort_by_key(|(TestDiscrim1(discrim), _)| *discrim);
        let expect_all =
            expect_all.iter().map(|(discrim, iso)| (*discrim, iso)).collect::<Vec<_>>();
        assert_eq!(map_vec, expect_all);
    }

    let mut world = system_test!(
        test_system.build(req_discrims, single_expects, expect_all);
    );

    let ent1 = world.create(crate::comps![@(crate) TestArch =>
        @(TestDiscrim1(11), Iso1(3)),
        @(TestDiscrim1(13), Iso1(5)),
    ]);
    world.get_global::<InitialEntities>().ent1 = Some(ent1);

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
fn test_full_isotope_discrim_write() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut iso1: impl system::WriteIsotope<TestArch, Iso1>,
        mut iso2: impl system::WriteIsotope<TestArch, Iso2>,
        #[dynec(global)] initials: &InitialEntities,
    ) {
        let ent1 = initials.ent1.as_ref().expect("ent1 is None");

        {
            let iso = iso1.try_get_mut(ent1, TestDiscrim1(11));
            let iso = iso.expect("was passed input");
            assert_eq!(iso, &mut Iso1(3));
            *iso = Iso1(23);
        }

        // should not panic on nonexistent storages
        {
            let iso = iso1.try_get_mut(ent1, TestDiscrim1(17));
            assert_eq!(iso, None);
        }

        // should update new storages
        {
            let iso = iso1.set(ent1, TestDiscrim1(19), Some(Iso1(29)));
            assert_eq!(iso, None);
        }

        // should return default value
        {
            let iso = iso2.try_get(ent1, TestDiscrim2(31));
            assert_eq!(iso.as_deref(), Some(&Iso2(-1)));
        }

        // should reset to default value
        {
            let iso = iso2.set(ent1, TestDiscrim2(37), None);
            assert_eq!(iso, Some(Iso2(41)));
        }
        {
            let iso = iso2.try_get(ent1, TestDiscrim2(37));
            dbg!(iso.as_deref());
            assert_eq!(iso.as_deref(), Some(&Iso2(-1)));
        }

        // should include new discriminants
        let map = iso1.get_all(ent1);
        let mut map_vec: Vec<(TestDiscrim1, &Iso1)> = map.collect();
        map_vec.sort_by_key(|(TestDiscrim1(discrim), _)| *discrim);
        assert_eq!(
            map_vec,
            vec![
                (TestDiscrim1(11), &Iso1(23)),
                (TestDiscrim1(13), &Iso1(5)),
                (TestDiscrim1(19), &Iso1(29)),
            ]
        );
    }

    let mut world = system_test!(test_system.build(););

    let ent1 = world.create(crate::comps![@(crate) TestArch =>
        @(TestDiscrim1(11), Iso1(3)),
        @(TestDiscrim1(13), Iso1(5)),
        @(TestDiscrim2(37), Iso2(41)),
    ]);
    world.get_global::<InitialEntities>().ent1 = Some(ent1);

    world.execute(&tracer::Log(log::Level::Trace));
}

#[test]
fn test_partial_isotope_discrim_write() {
    partial_isotope_discrim_write(
        vec![TestDiscrim1(7), TestDiscrim1(11), TestDiscrim1(17), TestDiscrim1(19)],
        vec![
            (0, Some(Iso1(2)), Some(None)),
            (1, Some(Iso1(3)), Some(Some(Iso1(23)))),
            (2, None, None),
            (3, None, Some(Some(Iso1(29)))),
        ],
        vec![(TestDiscrim1(11), Iso1(23)), (TestDiscrim1(19), Iso1(29))],
    );
}

#[test]
#[should_panic = "Cannot access isotope indexed by 42 because it is not in the list of requested \
                  discriminants"]
fn test_partial_isotope_discrim_write_panic() {
    partial_isotope_discrim_write(vec![TestDiscrim1(11)], vec![(42, None, None)], vec![]);
}

type SingleExpectUpdate = (usize, Option<Iso1>, Option<Option<Iso1>>);

fn partial_isotope_discrim_write(
    req_discrims: Vec<TestDiscrim1>,
    single_expect_updates: Vec<SingleExpectUpdate>,
    expect_all: Vec<(TestDiscrim1, Iso1)>,
) {
    #[system(dynec_as(crate))]
    fn test_system(
        #[dynec(param)] _req_discrims: &Vec<TestDiscrim1>,
        #[dynec(param)] single_expect_updates: &mut Vec<SingleExpectUpdate>,
        #[dynec(param)] expect_all: &Vec<(TestDiscrim1, Iso1)>,
        #[dynec(isotope(discrim = _req_discrims))] mut iso1: impl system::WriteIsotope<
            TestArch,
            Iso1,
            usize,
        >,
        #[dynec(global)] initials: &InitialEntities,
    ) {
        let ent1 = initials.ent1.as_ref().expect("ent1 is None");

        for (discrim, mut expect, update) in single_expect_updates.drain(..) {
            let iso = iso1.try_get_mut(ent1, discrim);
            assert_eq!(iso, expect.as_mut());
            if let Some(update) = update {
                iso1.set(ent1, discrim, update);
            }
        }

        // should only include requested discriminants
        let map = iso1.get_all(ent1);
        let map_vec: Vec<(TestDiscrim1, &Iso1)> = map.collect();
        let expect_all =
            expect_all.iter().map(|(discrim, iso)| (*discrim, iso)).collect::<Vec<_>>();
        assert_eq!(map_vec, expect_all);
    }

    let mut world =
        system_test!(test_system.build(req_discrims, single_expect_updates, expect_all););

    let ent1 = world.create(crate::comps![@(crate) TestArch =>
        @(TestDiscrim1(7), Iso1(2)),
        @(TestDiscrim1(11), Iso1(3)),
        @(TestDiscrim1(13), Iso1(5)),
    ]);
    world.get_global::<InitialEntities>().ent1 = Some(ent1);

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
        mut entity_creator: impl system::EntityCreator<TestArch>,
        #[dynec(global(maybe_uninit(TestArch)))] initials: &mut InitialEntities,
        #[dynec(global)] step: &Step,
    ) {
        match step {
            Step::Create => {
                initials.ent1 =
                    Some(entity_creator.create(crate::comps![@(crate) TestArch => Comp1(5)]));
            }
            Step::Access => {}
        }
    }

    #[system(dynec_as(crate))]
    fn comp_access_system(
        comp1: impl system::ReadSimple<TestArch, Comp1>,
        #[dynec(global)] initials: &InitialEntities,
        #[dynec(global)] step: &Step,
    ) {
        match step {
            Step::Create => {
                assert!(initials.ent1.is_none());
            }
            Step::Access => {
                let ent = initials.ent1.as_ref().expect("ent1 should have been set");
                comp1.try_get(ent).expect("ent1 should have been initialized");
            }
        }
    }

    #[system(dynec_as(crate), after(LatePartition))]
    fn late_comp_access_system(
        // component storage does not require maybe_uninit unless the component has something like `Option<Box<Self>>`
        comp1: impl system::ReadSimple<TestArch, Comp1>,
        #[dynec(global(maybe_uninit(TestArch)))] initials: &InitialEntities,
        #[dynec(global)] step: &Step,
    ) {
        match step {
            Step::Create => {
                let ent = initials.ent1.as_ref().expect("ent1 should have been set");
                assert!(comp1.try_get(ent).is_none(), "entity should be in pre-initialize state");
            }
            Step::Access => {
                let ent = initials.ent1.as_ref().expect("ent1 should have been set");
                comp1.try_get(ent).expect("ent1 should have been initialized");
            }
        }
    }

    let mut world = system_test!(comp_access_system.build(), late_comp_access_system.build(), entity_creator_system.build(););

    world.execute(&tracer::Log(log::Level::Trace));
    *world.get_global::<Step>() = Step::Access;
    world.execute(&tracer::Log(log::Level::Trace));

    let ent1 = {
        let initials = world.get_global::<InitialEntities>();
        let ent1 = initials.ent1.as_ref().expect("ent1 missing");
        ent1.clone()
    };
    let comp1 = world.get_simple::<TestArch, Comp1, _>(&ent1);
    assert_eq!(comp1, Some(&mut Comp1(5)));
}

#[test]
#[should_panic = "Scheduled systems have a cyclic dependency: "]
fn test_offline_create_conflict() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_creator: impl system::EntityCreator<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: impl system::ReadSimple<TestArch, Comp1>,
    ) {
        initials.ent1 = Some(entity_creator.create(crate::comps![@(crate) TestArch => Comp1(5)]));
    }

    let mut world = system_test!(test_system.build(););

    world.execute(&tracer::Log(log::Level::Trace));

    let ent1 = {
        let initials = world.get_global::<InitialEntities>();
        let ent1 = initials.ent1.as_ref().expect("ent1 missing");
        ent1.clone()
    };
    let comp1 = world.get_simple::<TestArch, Comp1, _>(&ent1);
    assert_eq!(comp1, Some(&mut Comp1(5)));
}

#[test]
fn test_offline_delete() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_deleter: impl system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: impl system::ReadSimple<TestArch, Comp1>,
    ) {
        entity_deleter.queue(initials.ent1.take().expect("ent1 missing"));
    }

    let mut world = system_test!(test_system.build(););
    let ent1 = world.create(crate::comps![@(crate) TestArch => Comp1(7)]);
    let ent1_weak = ent1.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().ent1 = Some(ent1);

    world.execute(&tracer::Log(log::Level::Trace));

    let comp1 = world.get_simple::<TestArch, Comp1, _>(&ent1_weak);
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
        mut entity_deleter: impl system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: impl system::ReadSimple<TestArch, Comp1>,
    ) {
        if let Some(ent) = initials.ent1.take() {
            *entity = Some(ent);
        }

        if let Some(ent) = entity {
            entity_deleter.queue(&*ent);
        }
    }

    let mut builder = world::Builder::new(0);
    builder.schedule(Box::new(test_system.build()));

    let mut world = builder.build();

    let ent1 = world.create(crate::comps![@(crate) TestArch => Comp1(7)]);
    let ent1_weak = ent1.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().ent1 = Some(ent1);

    world.execute(&tracer::Log(log::Level::Trace));

    let comp1 = world.get_simple::<TestArch, Comp1, _>(&ent1_weak);
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
        mut entity_deleter: impl system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: impl system::ReadSimple<TestArch, Comp1>,
    ) {
        if let Some(ent) = initials.ent1.take() {
            *entity = Some(ent);
        }

        if let Some(ent) = entity {
            entity_deleter.queue(&*ent);
        }
    }

    let mut builder = world::Builder::new(0);
    builder.schedule_thread_unsafe(Box::new(test_system.build()));

    let mut world = builder.build();

    let ent1 = world.create(crate::comps![@(crate) TestArch => Comp1(7)]);
    let ent1_weak = ent1.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().ent1 = Some(ent1);

    world.execute(&tracer::Log(log::Level::Trace));

    let comp1 = world.get_simple::<TestArch, Comp1, _>(&ent1_weak);
    assert_eq!(comp1, None);
}

#[test]
#[cfg_attr(
    any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ),
    should_panic = "Detected dangling strong reference to entity dynec::test_util::TestArch#1 in \
                    global state dynec::world::tests::InitialEntities. All strong references to \
                    an entity must be dropped before queuing for deletion and removing all \
                    finalizers."
)]
fn test_offline_delete_sync_global_leak() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_deleter: impl system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        _comp1: impl system::ReadSimple<TestArch, Comp1>,
    ) {
        entity_deleter.queue(initials.ent1.as_ref().expect("ent1 missing"));
    }

    let mut world = system_test!(test_system.build(););
    let ent1 = world.create(crate::comps![@(crate) TestArch => Comp1(7)]);
    let ent1_weak = ent1.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().ent1 = Some(ent1);

    world.execute(&tracer::Log(log::Level::Trace));

    let comp1 = world.get_simple::<TestArch, Comp1, _>(&ent1_weak);
    assert_eq!(comp1, None);
}

#[test]
#[cfg_attr(
    any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ),
    should_panic = "Detected dangling strong reference to entity dynec::test_util::TestArch#1 in \
                    global state dynec::world::tests::InitialEntities. All strong references to \
                    an entity must be dropped before queuing for deletion and removing all \
                    finalizers."
)]
fn test_offline_delete_unsync_global_leak() {
    #[system(dynec_as(crate), thread_local)]
    fn test_system(
        mut entity_deleter: impl system::EntityDeleter<TestArch>,
        #[dynec(global(thread_local))] initials: &mut InitialEntities,
        _comp1: impl system::ReadSimple<TestArch, Comp1>,
    ) {
        entity_deleter.queue(initials.ent1.as_ref().expect("ent1 missing"));
    }

    let mut builder = world::Builder::new(0);
    builder.schedule_thread_unsafe(Box::new(test_system.build()));

    let mut world = builder.build();

    let ent1 = world.create(crate::comps![@(crate) TestArch => Comp1(7)]);
    let ent1_weak = ent1.weak(world.get_global::<generation::StoreMap>());
    world.get_global_unsync::<InitialEntities>().ent1 = Some(ent1);

    world.execute(&tracer::Log(log::Level::Trace));

    let comp1 = world.get_simple::<TestArch, Comp1, _>(&ent1_weak);
    assert_eq!(comp1, None);
}

// TODO add tests for leaking from simple and isotope components

#[test]
fn test_offline_finalizer_delete() {
    #[system(dynec_as(crate))]
    fn test_system(
        mut entity_deleter: impl system::EntityDeleter<TestArch>,
        #[dynec(global)] initials: &mut InitialEntities,
        #[dynec(global)] deletion_flags: &deletion::Flags,
        mut comp_final: impl system::WriteSimple<TestArch, CompFinal>,
        _comp1: impl system::ReadSimple<TestArch, Comp1>,
    ) {
        let ent1 = initials.ent1.as_ref().expect("ent1 missing");
        if deletion_flags.get::<TestArch>(ent1.id()) {
            comp_final.set(ent1, None);
            initials.ent1 = None;
        } else {
            entity_deleter.queue(ent1);
        }
    }

    let mut world = system_test!(test_system.build(););
    let ent1 = world.create(crate::comps![@(crate) TestArch => Comp1(13), CompFinal]);
    let ent1_weak = ent1.weak(world.get_global::<generation::StoreMap>());
    world.get_global::<InitialEntities>().ent1 = Some(ent1);

    // first iteration
    world.execute(&tracer::Log(log::Level::Trace));

    let comp1 = world.get_simple::<TestArch, Comp1, _>(&ent1_weak);
    assert_eq!(comp1, Some(&mut Comp1(13)));

    // second iteration
    world.execute(&tracer::Log(log::Level::Trace));

    let comp1 = world.get_simple::<TestArch, Comp1, _>(&ent1_weak);
    assert_eq!(comp1, None);
}
