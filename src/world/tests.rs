use crate::{world, World};

#[test]
fn test_storage_init() {
    // #[crate::system]
    fn test_system() {}

    struct TestBundle;

    impl world::Bundle for TestBundle {
        // fn register(&self, builder: &mut world::Builder) { builder.schedule(test_system); }
    }
}
