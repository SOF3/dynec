//! Tests isotope storage access.

#![allow(clippy::ptr_arg)]

use crate::test_util::*;
use crate::{system, system_test, tracer, world};

fn isotope_discrim_read_test_system(
    mut iso1: impl system::access::isotope::Get<Arch = TestArch, Comp = IsoNoInit, Key = TestDiscrim1>,
    mut iso2: impl system::access::isotope::Get<Arch = TestArch, Comp = IsoWithInit, Key = TestDiscrim2>,
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

fn isotope_discrim_test_world(system: impl system::Sendable) -> world::World {
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
                  dynec::test_util::TestArch/dynec::test_util::isotope_comps::IsoNoInit"]
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
                  dynec::test_util::TestArch/dynec::test_util::isotope_comps::IsoNoInit"]
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
