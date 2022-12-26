use std::cell::Cell;
use std::env;
use std::rc::Rc;
use std::sync::atomic::{self, AtomicUsize};
use std::sync::Arc;

use super::*;
use crate::entity::referrer;
use crate::test_util::{self, AntiSemaphore, TestArch};
use crate::world::offline;
use crate::{comp, system, tracer, world};

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

struct TestSystem<D: ?Sized + 'static>(String, Box<D>);
impl<D: ?Sized> referrer::Referrer for TestSystem<D> {
    fn visit_type(_arg: &mut referrer::VisitTypeArg) {}
    fn visit_mut<V: referrer::VisitMutArg>(&mut self, _arg: &mut V) {}
}
impl<D: ?Sized> system::Descriptor for TestSystem<D> {
    fn get_spec(&self) -> system::Spec { dummy_spec(self.0.as_str()) }
    fn visit_type(&self, _arg: &mut referrer::VisitTypeArg) {} // no types to visit
    fn visit_mut(&mut self) -> referrer::AsObject<'_> { referrer::AsObject::of(self) }
}
type SendSystem = TestSystem<dyn Fn() + Send>;
impl system::Sendable for SendSystem {
    fn run(
        &mut self,
        _globals: &world::SyncGlobals,
        _components: &world::Components,
        _ealloc_shard_map: &mut ealloc::ShardMap,
        _offline_buffer: &mut offline::BufferShard,
    ) {
        self.1();
    }

    fn as_descriptor_mut(&mut self) -> &mut dyn system::Descriptor { self }
}

type UnsendSystem = TestSystem<dyn Fn()>;
impl system::Unsendable for UnsendSystem {
    fn run(
        &mut self,
        _sync_globals: &world::SyncGlobals,
        _unsync_globals: &mut world::UnsyncGlobals,
        _components: &world::Components,
        _ealloc_shard_map: &mut ealloc::ShardMap,
        _offline_buffer: &mut offline::BufferShard,
    ) {
        self.1();
    }

    fn as_descriptor_mut(&mut self) -> &mut dyn system::Descriptor { self }
}

struct Global1;
struct Global2;

#[comp(dynec_as(crate), of = TestArch)]
struct Comp1;
#[comp(dynec_as(crate), of = TestArch)]
struct Comp2;

#[derive(Debug, PartialEq, Eq, Hash)]
struct TestPartition(u32);

/// Counts the number of times some node is unmarked as runnable.
#[derive(Default)]
struct UnmarkCounterTracer(AtomicUsize);
#[dynec_codegen::tracer(dynec_as())]
impl Tracer for UnmarkCounterTracer {
    fn unmark_runnable(&self, _node: scheduler::Node) {
        self.0.fetch_add(1, atomic::Ordering::SeqCst);
    }
}

/// Collects the maximum concurrency.
#[derive(Default)]
struct MaxConcurrencyTracer {
    current: AtomicUsize,
    max:     AtomicUsize,
}
#[dynec_codegen::tracer(dynec_as())]
impl Tracer for MaxConcurrencyTracer {
    fn start_run_sendable(
        &self,
        _thread: tracer::Thread,
        _node: scheduler::Node,
        _debug_name: &str,
        _system: &mut dyn system::Sendable,
    ) {
        let value = self.current.fetch_add(1, atomic::Ordering::SeqCst);
        self.max.fetch_max(value + 1, atomic::Ordering::SeqCst);
    }

    fn end_run_sendable(
        &self,
        (): (),
        _thread: tracer::Thread,
        _node: scheduler::Node,
        _debug_name: &str,
        _system: &mut dyn system::Sendable,
    ) {
        self.current.fetch_sub(1, atomic::Ordering::SeqCst);
    }

    fn start_run_unsendable(
        &self,
        _thread: tracer::Thread,
        _node: scheduler::Node,
        _debug_name: &str,
        _system: &mut dyn system::Unsendable,
    ) {
        let value = self.current.fetch_add(1, atomic::Ordering::SeqCst);
        self.max.fetch_max(value + 1, atomic::Ordering::SeqCst);
    }

    fn end_run_unsendable(
        &self,
        (): (),
        _thread: tracer::Thread,
        _node: scheduler::Node,
        _debug_name: &str,
        _system: &mut dyn system::Unsendable,
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
#[dynec_codegen::tracer(dynec_as())]
impl Tracer for RunCounterTracer {
    fn start_run_sendable(
        &self,
        _thread: tracer::Thread,
        _node: scheduler::Node,
        _debug_name: &str,
        _system: &mut dyn system::Sendable,
    ) {
        self.send.fetch_add(1, atomic::Ordering::SeqCst);
    }

    fn start_run_unsendable(
        &self,
        _thread: tracer::Thread,
        _node: scheduler::Node,
        _debug_name: &str,
        _system: &mut dyn system::Unsendable,
    ) {
        self.unsend.fetch_add(1, atomic::Ordering::SeqCst);
    }
}

/// Tracks the start order of systems.
#[derive(Default)]
struct StartOrderTracer(Mutex<Vec<Node>>);
#[dynec_codegen::tracer(dynec_as())]
impl Tracer for StartOrderTracer {
    fn start_run_sendable(
        &self,
        _thread: tracer::Thread,
        node: scheduler::Node,
        _debug_name: &str,
        _system: &mut dyn system::Sendable,
    ) {
        let mut vec = self.0.lock();
        vec.push(node);
    }
    fn start_run_unsendable(
        &self,
        _thread: tracer::Thread,
        node: scheduler::Node,
        _debug_name: &str,
        _system: &mut dyn system::Unsendable,
    ) {
        let mut vec = self.0.lock();
        vec.push(node);
    }
}

#[test]
fn test_empty() {
    for concurrency in 0..3 {
        bootstrap(concurrency, || (), |_builder, [], []| {}, || |_| (), |()| {});
    }
}

#[test]
fn test_global_exclusion() {
    bootstrap(
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
    bootstrap(
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
    bootstrap(
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
    bootstrap(
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
    bootstrap(
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
    bootstrap(
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
    bootstrap(
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
    bootstrap(
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
    bootstrap(
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
    bootstrap(
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

#[test]
fn test_partition() {
    bootstrap(
        2,
        || (StartOrderTracer::default(),),
        |builder, [sys1, sys2], []| {
            builder.add_dependencies(
                vec![system::spec::Dependency::After(Box::new(TestPartition(0)))],
                Node::SendSystem(sys1),
            );
            builder.add_dependencies(
                vec![system::spec::Dependency::Before(Box::new(TestPartition(0)))],
                Node::SendSystem(sys2),
            );
        },
        || |_| (),
        |(StartOrderTracer(order),)| {
            assert_eq!(
                &order.into_inner()[..],
                &[Node::SendSystem(SendSystemIndex(1)), Node::SendSystem(SendSystemIndex(0))]
            );
        },
    );
}

#[test]
fn test_duplicate_partition() {
    bootstrap(
        2,
        || (StartOrderTracer::default(),),
        |builder, [sys1, sys2], []| {
            builder.add_dependencies(
                vec![
                    system::spec::Dependency::After(Box::new(TestPartition(0))),
                    system::spec::Dependency::After(Box::new(TestPartition(0))),
                ],
                Node::SendSystem(sys1),
            );
            builder.add_dependencies(
                vec![
                    system::spec::Dependency::Before(Box::new(TestPartition(0))),
                    system::spec::Dependency::Before(Box::new(TestPartition(0))),
                ],
                Node::SendSystem(sys2),
            );
        },
        || |_| (),
        |(StartOrderTracer(order),)| {
            assert_eq!(
                &order.into_inner()[..],
                &[Node::SendSystem(SendSystemIndex(1)), Node::SendSystem(SendSystemIndex(0))]
            );
        },
    );
}

#[test]
#[should_panic = "Scheduled systems have a cyclic dependency: thread-safe system #0 (SendSystem \
                  #0) -> partition #0 (TestPartition(0)) -> thread-safe system #0 (SendSystem #0)"]
fn test_conflicting_partition() {
    bootstrap(
        2,
        || (),
        |builder, [sys1], []| {
            builder.add_dependencies(
                vec![
                    system::spec::Dependency::After(Box::new(TestPartition(0))),
                    system::spec::Dependency::Before(Box::new(TestPartition(0))),
                ],
                Node::SendSystem(sys1),
            );
        },
        || |_| (),
        |()| {},
    );
}

// Make sure that thread-local systems have the same exclusion rules as thread-safe systems.
#[test]
fn test_thread_local_exclusion() {
    bootstrap(
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
    bootstrap(
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
    bootstrap(
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
fn bootstrap<const S: usize, const U: usize, T, C, R, V>(
    concurrency: usize,
    make_tracers: fn() -> T,
    customize: C,
    make_run: fn() -> R,
    verify: V,
) where
    C: Fn(&mut Builder, [SendSystemIndex; S], [UnsendSystemIndex; U]),
    R: Fn(Node) + Send + Sync + 'static,
    V: Fn(T),
    tracer::Aggregate<T>: Tracer,
{
    test_util::init();

    for i in 0..*CONCURRENT_TEST_REPETITIONS {
        log::trace!("Repeat test round {i}");

        let mut builder = Builder::new(concurrency);

        let run = Arc::new(make_run());

        let send_nodes: [SendSystemIndex; S] = (0..S)
            .map(|i| {
                let node_box = Arc::new(Mutex::new(None::<Node>));
                let (node, _spec) = builder.push_send_system(Box::new(TestSystem(
                    format!("SendSystem #{}", i),
                    Box::new({
                        let run = Arc::clone(&run);
                        let node_box = Arc::clone(&node_box);
                        move || {
                            let node_guard = node_box.try_lock().expect("node_box contention");
                            let &node = node_guard.as_ref().expect("node_box not populated");
                            run(node)
                        }
                    }) as Box<dyn Fn() + Send>,
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
                let (node, _spec) = builder.push_unsend_system(Box::new(TestSystem(
                    format!("UnsendSystem #{}", i),
                    Box::new({
                        let run = Arc::clone(&run);
                        let node_box = Rc::clone(&node_box);
                        move || {
                            let node = node_box.get();
                            let node = node.expect("node_box not populated");
                            run(node)
                        }
                    }) as Box<dyn Fn()>,
                )
                    as UnsendSystem));
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
            &mut rctrack::MaybeStoreMap::default(),
            &mut ealloc::Map::default(),
        );

        let tracer::Aggregate((_, rct, tracer::Aggregate(tracers))) = tracer;

        assert_eq!(rct.send.load(atomic::Ordering::SeqCst), S);
        assert_eq!(rct.unsend.load(atomic::Ordering::SeqCst), U);

        verify(tracers);
    }
}
