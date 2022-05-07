use super::*;
use crate::{system, world};

struct Tracer<Id>(Id, Box<dyn Fn() -> system::Spec + Send>);

impl<Id: Send> system::Sendable for Tracer<Id> {
    fn get_spec(&self) -> system::Spec { self.1() }

    fn run(&mut self, globals: &world::SendGlobals, components: &world::Components) {}
}

#[test]
fn test() {
    let mut builder = Builder::new(0);
    builder.push_send_system(Box::new(Tracer(
        1_usize,
        Box::new(|| system::Spec {
            debug_name:       String::from("1"),
            dependencies:     vec![],
            global_requests:  vec![],
            simple_requests:  vec![],
            isotope_requests: vec![],
        }),
    )));
    let scheduler = builder.build();
}
