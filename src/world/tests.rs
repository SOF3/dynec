use super::tracer;
use crate::{comp, global, system, system_test, Entity, TestArch, TestDiscrim1, TestDiscrim2};

#[comp(dynec_as(crate), of = TestArch)]
#[derive(Debug)]
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
#[derive(Debug)]
struct Comp5(i32);
#[comp(dynec_as(crate), of = TestArch, required, init = || Comp6(9))]
#[derive(Debug)]
struct Comp6(i32);

#[comp(dynec_as(crate), of = TestArch, isotope = TestDiscrim1)]
#[derive(Debug)]
struct Iso1(i32);
#[comp(dynec_as(crate), of = TestArch, isotope = TestDiscrim2)]
#[derive(Debug)]
struct Iso2(i32);
#[comp(dynec_as(crate), of = TestArch, isotope = TestDiscrim1)]
#[derive(Debug)]
struct Iso3(i32);

#[global(dynec_as(crate), initial)]
#[derive(Default)]
struct Aggregator {
    comp30_sum:     i32,
    comp41_product: i32,
}

#[global(dynec_as(crate), initial)]
#[derive(Default)]
struct InitialEntities {
    ent1: Option<Entity<TestArch>>,
}

#[system(dynec_as(crate))]
fn my_system(
    comp3: impl system::ReadSimple<TestArch, Comp3>,
    mut comp4: impl system::WriteSimple<TestArch, Comp4>,
    comp5: impl system::ReadSimple<TestArch, Comp5>,
    comp6: impl system::ReadSimple<TestArch, Comp6>,
    iso1: impl system::ReadIsotope<TestArch, Iso1>,
    #[dynec(global)] aggregator: &mut Aggregator,
    #[dynec(global)] initials: &InitialEntities,
) {
    aggregator.comp30_sum = 1;

    let ent1 = initials.ent1.as_ref().expect("ent1 is None");
    {
        let comp = comp4.get_mut(ent1);

        assert_eq!(comp.0, 14);
        assert_eq!(comp.1, 32);
        comp.1 += comp.0;
    }

    // TODO test iterators
}

#[test]
#[should_panic(expected = "The component dynec::world::tests::Comp2 cannot be retrieved because \
                           it is not used in any systems")]
fn test_dependencies_successful() {
    let mut world = system_test!(my_system.build(););
    let entity = world.create::<TestArch>(crate::comps![
        @(crate) TestArch => Comp1(1), Comp5(1), @(TestDiscrim1(9), Iso1(1)),
    ]);

    match world.get_simple::<TestArch, Comp4, _>(&entity) {
        Some(&mut Comp4(c40, c41)) => {
            assert_eq!(c40, 1 * 7);
            assert_eq!(c41, (1 + 2) * 8);
        }
        None => panic!("Comp4 is used in system_with_comp3_comp4_comp5"),
    }

    world.get_simple::<TestArch, Comp2, _>(&entity); // panic here
}

#[test]
#[should_panic(expected = "Cannot create an entity of type `dynec::test_util::TestArch` without \
                           explicitly passing a component of type `dynec::world::tests::Comp5`")]
fn test_dependencies_missing_required_simple() {
    let mut world = system_test!(my_system.build(););
    world.create::<TestArch>(crate::comps![@(crate) TestArch => Comp1(1)]);
}

#[test]
#[should_panic(expected = "Cannot create an entity of type `dynec::test_util::TestArch` without \
                           explicitly passing a component of type `dynec::world::tests::Comp1`, \
                           which is required for `dynec::world::tests::Comp2`")]
fn test_dependencies_missing_required_dep() {
    let mut world = system_test!(my_system.build(););
    world.create::<TestArch>(crate::comps![@(crate) TestArch => Comp5(1)]);
}

#[test]
fn test_world_run() {
    let mut world = system_test!(my_system.build(););

    let ent1 = world.create(crate::comps![@(crate) TestArch =>
        Comp1(2), Comp5(3), @(TestDiscrim1(9), Iso1(4)),
    ]);
    world.get_global::<InitialEntities>().ent1 = Some(ent1.clone());

    world.execute(&tracer::Log(log::Level::Trace));

    let aggregator = world.get_global::<Aggregator>();
    assert_eq!(aggregator.comp30_sum, 1);

    let comp = world.get_simple::<TestArch, Comp4, _>(ent1);
    assert_eq!(comp, Some(&mut Comp4(14, 46)));
}
