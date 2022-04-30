use crate::world;

#[test]
fn test_storage_init() {
    #[crate::system(dynec_as(crate))]
    fn test_system() {}

    struct TestBundle;

    impl world::Bundle for TestBundle {
        fn register(&self, builder: &mut world::Builder) {
            builder.schedule(Box::new(test_system.build()));
        }
    }

    let world = world::new([&TestBundle as &dyn world::Bundle]);
}
