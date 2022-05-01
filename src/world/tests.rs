use std::any::TypeId;

use crate::{component, system, world, Archetype};

enum TestArch {}
impl Archetype for TestArch {}

#[test]
fn test_storage_init() {
    #[component(dynec_as(crate), of = TestArch)]
    struct Comp1(i32);
    #[component(dynec_as(crate), of = TestArch, init = init_comp2/1)]
    struct Comp2(i32);
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

    fn init_comp2(c1: &Comp1) -> Comp2 { Comp2(c1.0 + 2) }

    #[system(dynec_as(crate))]
    fn test_system(
        comp3_read: system::Simple<TestArch, &Comp3>,
        comp4_read: system::Simple<TestArch, &mut Comp4>,
    ) {
    }

    struct TestBundle;

    impl world::Bundle for TestBundle {
        fn register(&self, builder: &mut world::Builder) {
            builder.schedule(Box::new(test_system.build()));
        }
    }

    let mut world = world::new([&TestBundle as &dyn world::Bundle]);

    let entity = world.create::<TestArch>(crate::components![@(crate) TestArch => Comp1(1)]);
}
