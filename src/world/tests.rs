use crate::{comp, system, system_test, Archetype};

enum TestArch {}
impl Archetype for TestArch {}

#[comp(dynec_as(crate), of = TestArch)]
struct Comp1(i32);

#[comp(dynec_as(crate), of = TestArch, init = init_comp2/1)]
struct Comp2(i32);
fn init_comp2(c1: &Comp1) -> Comp2 { Comp2(c1.0 + 2) }

#[comp(
    dynec_as(crate),
    of = TestArch,
    init = |c1: &Comp1, c2: &Comp2| Comp3(c1.0 * 3, c2.0 * 5),
)]
struct Comp3(i32, i32);

#[comp(
    dynec_as(crate),
    of = TestArch,
    init = |c1: &Comp1, c2: &Comp2| Comp4(c1.0 * 7, c2.0 * 8),
)]
struct Comp4(i32, i32);

#[comp(dynec_as(crate), of = TestArch, required)]
struct Comp5(i32);

#[comp(dynec_as(crate), of = TestArch, required, init = || Comp6(9))]
struct Comp6(i32);

#[system(dynec_as(crate))]
fn system_with_comp3_comp4_comp5(
    _comp3: system::Simple<TestArch, &Comp3>,
    _comp4: system::Simple<TestArch, &mut Comp4>,
    _comp5: system::Simple<TestArch, &Comp5>,
) {
}

#[test]
#[should_panic(expected = "The component dynec::world::tests::Comp2 cannot be retrieved because \
                           it is not used in any systems")]
fn test_dependencies_successful() {
    let mut world = system_test!(system_with_comp3_comp4_comp5.build(););
    let entity = world.create::<TestArch>(crate::comps![@(crate) TestArch => Comp1(1), Comp5(1)]);

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
#[should_panic(expected = "Cannot create an entity of type `dynec::world::tests::TestArch` \
                           without explicitly passing a component of type \
                           `dynec::world::tests::Comp5`")]
fn test_dependencies_missing_required_simple() {
    let mut world = system_test!(system_with_comp3_comp4_comp5.build(););
    world.create::<TestArch>(crate::comps![@(crate) TestArch => Comp1(1)]);
}

#[test]
#[should_panic(expected = "Cannot create an entity of type `dynec::world::tests::TestArch` \
                           without explicitly passing a component of type \
                           `dynec::world::tests::Comp1`, which is required for \
                           `dynec::world::tests::Comp2`")]
fn test_dependencies_missing_required_dep() {
    let mut world = system_test!(system_with_comp3_comp4_comp5.build(););
    world.create::<TestArch>(crate::comps![@(crate) TestArch => Comp5(1)]);
}

#[test]
fn test_world_run() {
    let world = system_test!(system_with_comp3_comp4_comp5.build(););
    // world.execute(&tracer::Log(log::Level::Trace));
}
