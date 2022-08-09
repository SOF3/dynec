use std::cell::Cell;
use std::env;
use std::rc::Rc;
use std::sync::atomic::{self, AtomicUsize};
use std::sync::Arc;

use super::*;
use crate::test_util::{self, AntiSemaphore};
use crate::world::{offline, tracer};
use crate::{comp, system, world, TestArch};

// Repeat concurrent tests to increase the chance of catching random bugs.
// However, do not rely on test repetitions to assert for behavior;
// use more synchronization where practical.
lazy_static::lazy_static! {
    static ref CONCURRENT_TEST_REPETITIONS: usize = (|| {
        if let Ok(count) = env::var("CONCURRENT_TEST_REPETITIONS") {
            if let Ok(count) = count.parse::<usize>() {
                return count;
            }
        }

        if env::var("RUST_LOG").is_ok() { 1 } else { 1000 }
    })();
}

/// `push_send_system` and `push_unsend_system` only check the `debug_name` field,
/// so other fields can be left empty.
fn dummy_spec(name: &str) -> system::Spec {
    system::Spec {
        debug_name:              name.to_string(),
        dependencies:            vec![],
        global_requests:         vec![],
        simple_requests:         vec![],
        isotope_requests:        vec![],
        entity_creator_requests: vec![],
    }
}

struct SendSystem(String, Box<dyn Fn() + Send>);
impl system::Sendable for SendSystem {
    fn get_spec(&self) -> system::Spec { dummy_spec(self.0.as_str()) }
    fn run(
        &mut self,
        globals: &world::SyncGlobals,
        components: &world::Components,
        ealloc_shard_map: &mut ealloc::ShardMap,
        offline_buffer: &mut offline::BufferShard,
    ) {
        self.1();
    }
}

struct UnsendSystem(String, Box<dyn Fn()>);
impl system::Unsendable for UnsendSystem {
    fn get_spec(&self) -> system::Spec { dummy_spec(self.0.as_str()) }
    fn run(
        &mut self,
        sync_globals: &world::SyncGlobals,
        unsync_globals: &mut world::UnsyncGlobals,
        components: &world::Components,
        ealloc_shard_map: &mut ealloc::ShardMap,
        offline_buffer: &mut offline::BufferShard,
    ) {
        self.1();
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

    fn start_run_unsendable(
        &self,
        thread: tracer::Thread,
        node: world::ScheduleNode,
        debug_name: &str,
        system: &mut dyn system::Unsendable,
    ) {
        let value = self.current.fetch_add(1, atomic::Ordering::SeqCst);
        self.max.fetch_max(value + 1, atomic::Ordering::SeqCst);
    }

    fn end_run_unsendable(
        &self,
        thread: tracer::Thread,
        node: world::ScheduleNode,
        debug_name: &str,
        system: &mut dyn system::Unsendable,
    ) {
        self.current.fetch_sub(1, atomic::Ordering::SeqCst);
    }
}

/// Counts the number of systems run.
#[derive(Default)]
struct RunCounterTracer {
    send:   AtomicUsize,
    unsend: AtomicUsize,
}
impl world::Tracer for RunCounterTracer {
    fn start_run_sendable(
        &self,
        thread: tracer::Thread,
        node: world::ScheduleNode,
        debug_name: &str,
        system: &mut dyn system::Sendable,
    ) {
        self.send.fetch_add(1, atomic::Ordering::SeqCst);
    }

    fn start_run_unsendable(
        &self,
        thread: tracer::Thread,
        node: world::ScheduleNode,
        debug_name: &str,
        system: &mut dyn system::Unsendable,
    ) {
        self.unsend.fetch_add(1, atomic::Ordering::SeqCst);
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

#[test]
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

// Make sure that thread-local systems have the same exclusion rules as thread-safe systems.
#[test]
fn test_thread_local_exclusion() {
    test_bootstrap(
        1,
        || (UnmarkCounterTracer::default(), MaxConcurrencyTracer::default()),
        |builder, [sys1], [sys2]| {
            builder.use_resource(
                Node::SendSystem(sys1),
                ResourceType::Isotope {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp1>(),
                },
                ResourceAccess { mutable: true, discrim: Some(vec![1, 2]) },
            );
            builder.use_resource(
                Node::UnsendSystem(sys2),
                ResourceType::Isotope {
                    arch: DbgTypeId::of::<TestArch>(),
                    comp: DbgTypeId::of::<Comp1>(),
                },
                ResourceAccess { mutable: true, discrim: Some(vec![2, 3]) },
            );
        },
        || |_| {},
        |(uct, mct)| {
            let unmark_count = uct.0.into_inner();
            assert_eq!(
                unmark_count, 1,
                "Expected thread-local and thread-safe systems to still conform to resource \
                 exclusion"
            );

            let max_concurrency = mct.max.into_inner();
            assert_eq!(max_concurrency, 1, "Expected 2 systems to run concurrently");
        },
    );
}

#[test]
fn test_zero_concurrency_single_send() {
    test_bootstrap(
        0,
        || (MaxConcurrencyTracer::default(),),
        |_builder, [_sys], []| {},
        || |_| {},
        |(mct,)| {
            let max_concurrency = mct.max.into_inner();
            assert_eq!(max_concurrency, 1, "Expected single send system to run");
        },
    );
}

#[test]
fn test_zero_concurrency_single_unsend() {
    test_bootstrap(
        0,
        || (MaxConcurrencyTracer::default(),),
        |_builder, [], [_sys]| {},
        || |_| {},
        |(mct,)| {
            let max_concurrency = mct.max.into_inner();
            assert_eq!(max_concurrency, 1, "Expected single unsend system to run");
        },
    );
}

/// Bootstraps a test function for the scheduler.
///
/// This function performs the following:
/// - Initialize the logger if it is missing.
/// - Repeat the test for [`CONCURRENT_TEST_REPETITIONS`] iterations.
/// - Schedules `S` thread-safe systems and `U` thread-unsafe systems to the scheduler,
///   calling `make_run` for every iteration to create a shared runner for all systems.
///   This means any values declared in the first layer of the `make_run` closure
///   are going to be shared among all systems in the same iteration,
///   while values declared in the second layer are local to a specific system run.
///   The inner closure receives the system node ID.
/// - Runs `customize` to setup system requests.
///   The second and third parameters of `customize` are arrays,
///   the respective lengths of which specify
///   the number of thread-safe and thread-unsafe systems to schedule.
///   The function is called with the node IDs of the corresponding systems.
///   Therefore, by writing the second and third parameters as array patterns,
///   the sizes `S` and `U` can be automatically inferred.
/// - Builds a new scheduler from the information above.
/// - Executes the built scheduler with a TRACE-level tracer,
///   along with the tuple of schedulers returned by `make_tracers`.
/// - Calls `verify` with the tuple of tracers to verify that the test has succeeded.
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
    test_util::init();

    for i in 0..*CONCURRENT_TEST_REPETITIONS {
        log::trace!("Repeat test round {i}");

        let mut builder = Builder::new(concurrency);

        let run = Arc::new(make_run());

        let send_nodes: [SendSystemIndex; S] = (0..S)
            .map(|i| {
                let node_box = Arc::new(Mutex::new(None::<Node>));
                let (node, spec) = builder.push_send_system(Box::new(SendSystem(
                    format!("SendSystem #{}", i),
                    Box::new({
                        let run = Arc::clone(&run);
                        let node_box = Arc::clone(&node_box);
                        move || {
                            let node_guard = node_box.try_lock().expect("node_box contention");
                            let &node = node_guard.as_ref().expect("node_box not populated");
                            run(node)
                        }
                    }),
                )));
                {
                    let mut node_guard = node_box.try_lock().expect("node_box contention");
                    *node_guard = Some(node);
                }
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
                let node_box = Rc::new(Cell::new(None::<Node>));
                let (node, spec) = builder.push_unsend_system(Box::new(UnsendSystem(
                    format!("UnsendSystem #{}", i),
                    Box::new({
                        let run = Arc::clone(&run);
                        let node_box = Rc::clone(&node_box);
                        move || {
                            let node = node_box.get();
                            let node = node.expect("node_box not populated");
                            run(node)
                        }
                    }),
                )));
                node_box.set(Some(node));
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

        let tracer = tracer::Aggregate((
            tracer::Log(log::Level::Trace),
            RunCounterTracer::default(),
            tracer::Aggregate(make_tracers()),
        ));

        scheduler.execute(
            &tracer,
            &mut world::Components::empty(),
            &mut world::SyncGlobals::empty(),
            &mut world::UnsyncGlobals::empty(),
            &mut ealloc::Map::default(),
        );

        let tracer::Aggregate((_, rct, tracer::Aggregate(tracers))) = tracer;

        assert_eq!(rct.send.load(atomic::Ordering::SeqCst), S);
        assert_eq!(rct.unsend.load(atomic::Ordering::SeqCst), U);

        verify(tracers);
    }
}
