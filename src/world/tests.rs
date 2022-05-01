use crate::{component, system, world, Archetype};

enum TestArch {}
impl Archetype for TestArch {}

#[component(dynec_as(crate), of = TestArch)]
struct Comp1(i32);

#[component(dynec_as(crate), of = TestArch, init = init_comp2/1)]
struct Comp2(i32);
fn init_comp2(c1: &Comp1) -> Comp2 { Comp2(c1.0 + 2) }

#[component(
    dynec_as(crate),
    of = TestArch,
    init = |c1: &Comp1, c2: &Comp2| Comp3(c1.0 * 3, c2.0 * 5),
)]
struct Comp3(i32, i32);

#[component(
    dynec_as(crate),
    of = TestArch,
    init = |c1: &Comp1, c2: &Comp2| Comp4(c1.0 * 7, c2.0 * 8),
)]
struct Comp4(i32, i32);

#[component(dynec_as(crate), of = TestArch, required)]
struct Comp5(i32);

#[system(dynec_as(crate))]
fn system_with_comp3_comp4_comp5(
    _comp3: system::Simple<TestArch, &Comp3>,
    _comp4: system::Simple<TestArch, &mut Comp4>,
    _comp5: system::Simple<TestArch, &Comp5>,
) {
}

macro_rules! make_world {
    ($($systems:expr),* $(,)?) => {{
        struct AnonymousBundle;

        impl world::Bundle for AnonymousBundle {
            fn register(&self, builder: &mut world::Builder) {
                $(
                    builder.schedule(Box::new($systems));
                )*
            }
        }

        world::new([&AnonymousBundle as &dyn world::Bundle])
    }}
}

#[test]
fn test_dependencies_successful() {
    let mut world = make_world!(system_with_comp3_comp4_comp5.build());
    world.create::<TestArch>(crate::components![@(crate) TestArch => Comp1(1), Comp5(1)]);
}

#[test]
#[should_panic(expected = "Cannot create an entity of type `dynec::world::tests::TestArch` \
                           without explicitly passing a component of type \
                           `dynec::world::tests::Comp5`")]
fn test_dependencies_missing_required_simple() {
    let mut world = make_world!(system_with_comp3_comp4_comp5.build());
    world.create::<TestArch>(crate::components![@(crate) TestArch => Comp1(1)]);
}

#[test]
#[should_panic(expected = "Cannot create an entity of type `dynec::world::tests::TestArch` \
                           without explicitly passing a component of type \
                           `dynec::world::tests::Comp1`, which is required for \
                           `dynec::world::tests::Comp2`")]
fn test_dependencies_missing_required_dep() {
    let mut world = make_world!(system_with_comp3_comp4_comp5.build());
    world.create::<TestArch>(crate::components![@(crate) TestArch => Comp5(1)]);
}
