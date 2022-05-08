use std::sync::atomic::{self, AtomicUsize};
use std::sync::Arc;

use parking_lot::Once;

use super::*;
use crate::test_util::AntiSemaphore;
use crate::world::tracer;
use crate::{comp, system, world, TestArch};

// Repeat concurrent tests to increase the chance of catching random bugs.
// However, do not rely on test repetitions to assert for behavior;
// use more synchronization where practical.
const CONCURRENT_TEST_REPETITIONS: usize = if option_env!("RUST_LOG").is_some() { 1 } else { 100 };

/// `push_send_system` and `push_unsend_system` only check the `debug_name` field,
/// so other fields can be left empty.
fn dummy_spec(name: &str) -> system::Spec {
    system::Spec {
        debug_name:       name.to_string(),
        dependencies:     vec![],
        global_requests:  vec![],
        simple_requests:  vec![],
        isotope_requests: vec![],
    }
}

struct SendSystem(String, Box<dyn Fn() + Send>);
impl system::Sendable for SendSystem {
    fn get_spec(&self) -> system::Spec { dummy_spec(self.0.as_str()) }
    fn run(&mut self, globals: &world::SyncGlobals, components: &world::Components) { self.1(); }
}

struct UnsendSystem(String, Box<dyn Fn() + Send>);
impl system::Unsendable for UnsendSystem {
    fn get_spec(&self) -> system::Spec { dummy_spec(self.0.as_str()) }
    fn run(
        &mut self,
        sync_globals: &world::SyncGlobals,
        unsync_globals: &world::UnsyncGlobals,
        components: &world::Components,
    ) {
    }
}

struct Global1;
struct Global2;

#[comp(dynec_as(crate), of = TestArch)]
struct Comp1;
#[comp(dynec_as(crate), of = TestArch)]
struct Comp2;

/// Counts the number of times some node is unmarked as runnable.
#[derive(Default)]
struct UnmarkCounterTracer(AtomicUsize);
impl world::Tracer for UnmarkCounterTracer {
    fn unmark_runnable(&self, node: world::ScheduleNode) {
        self.0.fetch_add(1, atomic::Ordering::SeqCst);
    }
}
/// Collects the maximum concurrency.
#[derive(Default)]
struct MaxConcurrencyTracer {
    current: AtomicUsize,
    max:     AtomicUsize,
}
impl world::Tracer for MaxConcurrencyTracer {
    fn start_run_sendable(
        &self,
        thread: tracer::Thread,
        node: world::ScheduleNode,
        debug_name: &str,
        system: &mut dyn system::Sendable,
    ) {
        let value = self.current.fetch_add(1, atomic::Ordering::SeqCst);
        self.max.fetch_max(value + 1, atomic::Ordering::SeqCst);
    }

    fn end_run_sendable(
        &self,
        thread: tracer::Thread,
        node: world::ScheduleNode,
        debug_name: &str,
        system: &mut dyn system::Sendable,
    ) {
        self.current.fetch_sub(1, atomic::Ordering::SeqCst);
    }
}

#[test]
fn test_global_exclusion() {
    test_bootstrap(
        2,
        || (UnmarkCounterTracer::default(), MaxConcurrencyTracer::default()),
        |builder, [sys1, sys2, sys3], []| {
            builder.use_resource(
                Node::SendSystem(sys1),
                ResourceType::Global(DbgTypeId::of::<Global1>()),
                ResourceAccess { mutable: true, discrim: None },
            );
            builder.use_resource(
                Node::SendSystem(sys2),
                ResourceType::Global(DbgTypeId::of::<Global1>()),
                ResourceAccess { mutable: true, discrim: None },
            );
            builder.use_resource(
                Node::SendSystem(sys3),
                ResourceType::Global(DbgTypeId::of::<Global1>()),
                ResourceAccess { mutable: false, discrim: None }, // mutable: false here
            );
        },
        || |_| (),
        |(uct, mct)| {
            let unmark_count = uct.0.into_inner();
            assert_eq!(
                unmark_count, 3,
                "Expected resource exclusion to unmark all other runnable nodes"
            );

            let max_concurrency = mct.max.into_inner();
            assert_eq!(max_concurrency, 1, "Expected resource exclusion to deny concurrency");
        },
    );
}

fn test_different_global_exclusion() {
    test_bootstrap(
        2,
        || (UnmarkCounterTracer::default(), MaxConcurrencyTracer::default()),
        |builder, [sys1, sys2], []| {
            builder.use_resource(
                Node::SendSystem(sys1),
                ResourceType::Global(DbgTypeId::of::<Global1>()),
                ResourceAccess { mutable: true, discrim: None },
            );
            builder.use_resource(
                Node::SendSystem(sys2),
                ResourceType::Global(DbgTypeId::of::<Global2>()),
                ResourceAccess { mutable: true, discrim: None },
            );
        },
        || {
            let asem = AntiSemaphore::new(2);
            move |_| asem.wait()
        },
        |(uct, mct)| {
            let unmark_count = uct.0.into_inner();
            assert_eq!(unmark_count, 0, "Expected no resource exclusion on different globals");

            let max_concurrency = mct.max.into_inner();
            assert_eq!(max_concurrency, 2, "Expected 2 systems to run concurrently");
        },
    );
}

#[test]
fn test_global_share() {
    test_bootstrap(
        2,
        || (UnmarkCounterTracer::default(), MaxConcurrencyTracer::default()),
        |builder, [sys1, sys2, sys3, sys4], []| {
            for sys in [sys1, sys2, sys3, sys4] {
                builder.use_resource(
                    Node::SendSystem(sys),
                    ResourceType::Global(DbgTypeId::of::<Global1>()),
                    ResourceAccess { mutable: false, discrim: None },
                );
            }
        },
        || {
            let asem = AntiSemaphore::new(2);
            move |_| asem.wait()
        },
        |(uct, mct)| {
            let unmark_count = uct.0.into_inner();
            assert_eq!(unmark_count, 0, "Expected no exclusion");

            let max_concurrency = mct.max.into_inner();
            assert_eq!(max_concurrency, 2, "Expected 2 systems to run concurrently");
        },
    );
}

#[test]
fn test_simple_exclusion() {
    test_bootstrap(
        2,
        || (UnmarkCounterTracer::default(), MaxConcurrencyTracer::default()),
        |builder, [sys1, sys2, sys3], []| {
            builder.use_resource(
                Node::SendSystem(sys1),
                ResourceType::Simple {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp1>(),
                },
                ResourceAccess { mutable: true, discrim: None },
            );
            builder.use_resource(
                Node::SendSystem(sys2),
                ResourceType::Simple {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp1>(),
                },
                ResourceAccess { mutable: true, discrim: None },
            );
            builder.use_resource(
                Node::SendSystem(sys3),
                ResourceType::Simple {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp1>(),
                },
                ResourceAccess { mutable: false, discrim: None }, // mutable: false here
            );
        },
        || |_| (),
        |(uct, mct)| {
            let unmark_count = uct.0.into_inner();
            assert_eq!(
                unmark_count, 3,
                "Expected resource exclusion to unmark all other runnable nodes"
            );

            let max_concurrency = mct.max.into_inner();
            assert_eq!(max_concurrency, 1, "Expected resource exclusion to deny concurrency");
        },
    );
}

#[test]
fn test_different_simple_exclusion() {
    test_bootstrap(
        2,
        || (UnmarkCounterTracer::default(), MaxConcurrencyTracer::default()),
        |builder, [sys1, sys2], []| {
            builder.use_resource(
                Node::SendSystem(sys1),
                ResourceType::Simple {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp1>(),
                },
                ResourceAccess { mutable: true, discrim: None },
            );
            builder.use_resource(
                Node::SendSystem(sys2),
                ResourceType::Simple {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp2>(),
                },
                ResourceAccess { mutable: true, discrim: None },
            );
        },
        || {
            let asem = AntiSemaphore::new(2);
            move |_| asem.wait()
        },
        |(uct, mct)| {
            let unmark_count = uct.0.into_inner();
            assert_eq!(unmark_count, 0, "Expected no resource exclusion on different components");

            let max_concurrency = mct.max.into_inner();
            assert_eq!(max_concurrency, 2, "Expected 2 systems to run concurrently");
        },
    );
}

#[test]
fn test_simple_share() {
    test_bootstrap(
        2,
        || (UnmarkCounterTracer::default(), MaxConcurrencyTracer::default()),
        |builder, [sys1, sys2, sys3, sys4], []| {
            for sys in [sys1, sys2, sys3, sys4] {
                builder.use_resource(
                    Node::SendSystem(sys),
                    ResourceType::Simple {
                        arch: DbgTypeId::of::<TestArch>(),
                        comp: DbgTypeId::of::<Comp1>(),
                    },
                    ResourceAccess { mutable: false, discrim: None },
                );
            }
        },
        || {
            let asem = AntiSemaphore::new(2);
            move |_| asem.wait()
        },
        |(uct, mct)| {
            let unmark_count = uct.0.into_inner();
            assert_eq!(unmark_count, 0, "Expected no exclusion");

            let max_concurrency = mct.max.into_inner();
            assert_eq!(max_concurrency, 2, "Expected 2 systems to run concurrently");
        },
    );
}

#[test]
fn test_isotope_exclusion() {
    test_bootstrap(
        2,
        || (UnmarkCounterTracer::default(), MaxConcurrencyTracer::default()),
        |builder, [sys1, sys2, sys3], []| {
            builder.use_resource(
                Node::SendSystem(sys1),
                ResourceType::Isotope {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp1>(),
                },
                ResourceAccess { mutable: true, discrim: None },
            );
            builder.use_resource(
                Node::SendSystem(sys2),
                ResourceType::Isotope {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp1>(),
                },
                ResourceAccess { mutable: true, discrim: None },
            );
            builder.use_resource(
                Node::SendSystem(sys3),
                ResourceType::Isotope {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp1>(),
                },
                ResourceAccess { mutable: false, discrim: None }, // mutable: false here
            );
        },
        || |_| (),
        |(uct, mct)| {
            let unmark_count = uct.0.into_inner();
            assert_eq!(
                unmark_count, 3,
                "Expected resource exclusion to unmark all other runnable nodes"
            );

            let max_concurrency = mct.max.into_inner();
            assert_eq!(max_concurrency, 1, "Expected resource exclusion to deny concurrency");
        },
    );
}

#[test]
fn test_intersecting_isotope_exclusion() {
    test_bootstrap(
        3,
        || (UnmarkCounterTracer::default(), MaxConcurrencyTracer::default()),
        |builder, [sys1, sys2, sys3], []| {
            builder.use_resource(
                Node::SendSystem(sys1),
                ResourceType::Isotope {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp1>(),
                },
                ResourceAccess { mutable: true, discrim: Some(vec![1, 2]) },
            );
            builder.use_resource(
                Node::SendSystem(sys2),
                ResourceType::Isotope {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp1>(),
                },
                ResourceAccess { mutable: true, discrim: Some(vec![2, 3]) },
            );
            builder.use_resource(
                Node::SendSystem(sys3),
                ResourceType::Isotope {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp1>(),
                },
                ResourceAccess { mutable: false, discrim: Some(vec![3, 4]) }, // mutable: false here
            );
        },
        || {
            let asem = AntiSemaphore::new(2);
            move |node| {
                if matches!(node, Node::SendSystem(SendSystemIndex(0 | 2))) {
                    asem.wait();
                }
            }
        },
        |(uct, mct)| {
            let unmark_count = uct.0.into_inner();
            assert_eq!(
                unmark_count, 1,
                "Expected resource exclusion to unmark only exclusive nodes"
            );

            let max_concurrency = mct.max.into_inner();
            assert_eq!(
                max_concurrency, 2,
                "Expected [1, 2] and [3, 4] systems to run concurrently"
            );
        },
    );
}

#[test]
fn test_different_isotope_exclusion() {
    test_bootstrap(
        2,
        || (UnmarkCounterTracer::default(), MaxConcurrencyTracer::default()),
        |builder, [sys1, sys2], []| {
            builder.use_resource(
                Node::SendSystem(sys1),
                ResourceType::Isotope {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp1>(),
                },
                ResourceAccess { mutable: true, discrim: None },
            );
            builder.use_resource(
                Node::SendSystem(sys2),
                ResourceType::Isotope {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp2>(),
                },
                ResourceAccess { mutable: true, discrim: None },
            );
        },
        || {
            let asem = AntiSemaphore::new(2);
            move |_| asem.wait()
        },
        |(uct, mct)| {
            let unmark_count = uct.0.into_inner();
            assert_eq!(unmark_count, 0, "Expected no resource exclusion on different components");

            let max_concurrency = mct.max.into_inner();
            assert_eq!(max_concurrency, 2, "Expected 2 systems to run concurrently");
        },
    );
}

#[test]
fn test_isotope_share() {
    test_bootstrap(
        2,
        || (UnmarkCounterTracer::default(), MaxConcurrencyTracer::default()),
        |builder, [sys1, sys2, sys3, sys4], []| {
            for sys in [sys1, sys2, sys3, sys4] {
                builder.use_resource(
                    Node::SendSystem(sys),
                    ResourceType::Isotope {
                        arch: DbgTypeId::of::<TestArch>(),
                        comp: DbgTypeId::of::<Comp1>(),
                    },
                    ResourceAccess { mutable: false, discrim: None },
                );
            }
        },
        || {
            let asem = AntiSemaphore::new(2);
            move |_| asem.wait()
        },
        |(uct, mct)| {
            let unmark_count = uct.0.into_inner();
            assert_eq!(unmark_count, 0, "Expected no exclusion");

            let max_concurrency = mct.max.into_inner();
            assert_eq!(max_concurrency, 2, "Expected 2 systems to run concurrently");
        },
    );
}

fn test_bootstrap<const S: usize, const U: usize, T, C, R, V>(
    concurrency: usize,
    make_tracers: fn() -> T,
    customize: C,
    make_run: fn() -> R,
    verify: V,
) where
    C: Fn(&mut Builder, [SendSystemIndex; S], [UnsendSystemIndex; U]),
    R: Fn(Node) + Send + Sync + 'static,
    V: Fn(T),
    tracer::Aggregate<T>: world::Tracer,
{
    static SET_LOGGER_ONCE: Once = Once::new();
    SET_LOGGER_ONCE.call_once(env_logger::init);

    for _ in 0..CONCURRENT_TEST_REPETITIONS {
        let mut builder = Builder::new(concurrency);

        let run = Arc::new(make_run());

        let send_nodes: [SendSystemIndex; S] = (0..S)
            .map(|i| {
                let (node, spec) = builder.push_send_system(Box::new(SendSystem(
                    format!("SendSystem #{}", i),
                    Box::new({
                        let run = Arc::clone(&run);
                        move || run(Node::SendSystem(SendSystemIndex(i)))
                    }),
                )));
                match node {
                    Node::SendSystem(index) => index,
                    _ => unreachable!(),
                }
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("S == S");
        let unsend_nodes: [UnsendSystemIndex; U] = (0..U)
            .map(|i| {
                let (node, spec) = builder.push_unsend_system(Box::new(UnsendSystem(
                    format!("UnsendSystem #{}", i),
                    Box::new({
                        let run = Arc::clone(&run);
                        move || run(Node::UnsendSystem(UnsendSystemIndex(i)))
                    }),
                )));
                match node {
                    Node::UnsendSystem(index) => index,
                    _ => unreachable!(),
                }
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("U == U");

        customize(&mut builder, send_nodes, unsend_nodes);

        let mut scheduler = builder.build();

        let tracer =
            tracer::Aggregate((tracer::Log(log::Level::Trace), tracer::Aggregate(make_tracers())));

        scheduler.execute(
            &tracer,
            &world::Components::empty(),
            &world::SyncGlobals::empty(),
            &mut world::UnsyncGlobals::empty(),
        );

        let tracer::Aggregate((_, tracer::Aggregate(tracers))) = tracer;

        verify(tracers);
    }
}
