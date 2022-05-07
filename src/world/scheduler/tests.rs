use super::*;
use crate::system::{self, spec};
use crate::world;

struct DummySystem(String);

impl system::Sendable for DummySystem {
    /// `push_send_system` only checks the `debug_name` field, other fields can be left empty
    fn get_spec(&self) -> system::Spec {
        system::Spec {
            debug_name:       self.0.clone(),
            dependencies:     vec![],
            global_requests:  vec![],
            simple_requests:  vec![],
            isotope_requests: vec![],
        }
    }

    fn run(&mut self, globals: &world::SyncGlobals, components: &world::Components) {}
}

struct Global1;

impl Global1 {
    fn request(mutable: bool) -> spec::GlobalRequest {
        spec::GlobalRequest {
            ty: DbgTypeId::of::<Global1>(),
            initial: spec::GlobalInitial::Sync(|| Box::new(Global1)),
            mutable,
        }
    }
}

enum SystemEvent {
    StartRun(String),
    EndRun(String),
}

#[test]
fn test_global_concurrency() {
    env_logger::init();

    let mut builder = Builder::new(2);

    let (sys1, _) = builder.push_send_system(Box::new(DummySystem(String::from("1"))));
    let (sys2, _) = builder.push_send_system(Box::new(DummySystem(String::from("2"))));

    builder.use_resource(
        sys1,
        ResourceType::Global(DbgTypeId::of::<Global1>()),
        ResourceAccess { mutable: true, discrim: None },
    );
    builder.use_resource(
        sys2,
        ResourceType::Global(DbgTypeId::of::<Global1>()),
        ResourceAccess { mutable: true, discrim: None },
    );

    let mut scheduler = builder.build();

    let tracer = world::tracer::Log(log::Level::Trace);
    scheduler.execute(
        &tracer,
        &world::Components::empty(),
        &world::SyncGlobals::empty(),
        &mut world::UnsyncGlobals::empty(),
    );
}
