use parking_lot::{Condvar, Mutex, MutexGuard};

use super::planner::StealResult;
use super::state::SyncState;
use super::{Node, Planner, Topology, UnsendArgs};
use crate::entity::{ealloc, rctrack};
use crate::tracer::{self, Tracer};
use crate::world::{self, offline, WorldMut};

pub(crate) struct Executor {
    thread_pool:               Option<rayon::ThreadPool>,
    concurrency:               usize,
    pub(crate) offline_buffer: offline::Buffer,
}

impl Executor {
    /// Builds a new executor with the given `concurrency`.
    ///
    /// Note that `concurrency` only specifies the number of worker threads.
    /// The main thread is not considered a worker thread.
    /// Therefore, it is valid to set a concurrency of 0,
    /// especially in environments where threading is not supported.
    pub(crate) fn new(concurrency: usize) -> Self {
        Self {
            thread_pool: (concurrency > 0).then(|| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(concurrency)
                    .thread_name(|i| format!("dynec executor #{}", i))
                    .build()
                    .expect("Failed to create thread pool")
            }),
            concurrency,
            offline_buffer: offline::Buffer::new(concurrency + 1),
        }
    }

    #[allow(clippy::too_many_arguments)] // FIXME
    pub(crate) fn execute_full_cycle(
        &mut self,
        tracer: &impl Tracer,
        topology: &Topology,
        planner: &mut Mutex<Planner>,
        sync_state: &mut SyncState,
        components: &mut world::Components,
        sync_globals: &mut world::SyncGlobals,
        rctrack: &mut rctrack::MaybeStoreMap,
        mut unsend: UnsendArgs<'_>,
        ealloc_map: &mut ealloc::Map,
    ) {
        let condvar = Condvar::new();

        planner.get_mut().clone_from(topology.initial_planner());

        let cycle_context = tracer.start_cycle();

        for &index in &planner.get_mut().send_runnable {
            tracer.mark_runnable(Node::SendSystem(index));
        }
        for &index in &planner.get_mut().unsend_runnable {
            tracer.mark_runnable(Node::UnsendSystem(index));
        }

        for &index in &topology.depless_pars {
            let node = Node::Partition(index);
            let partition = &*topology.partitions.get(index.0).expect("invalid node index").0;
            tracer.partition(node, partition);
        }

        let context = Context { topology, planner, condvar: &condvar };

        let prepare_ealloc_shards_context = tracer.start_prepare_ealloc_shards();
        let mut ealloc_shards = ealloc_map.shards(self.concurrency + 1);
        tracer.end_prepare_ealloc_shards(prepare_ealloc_shards_context);

        let send = SendArgs { state: sync_state, components, globals: sync_globals };

        let deadlock_counter = DeadlockCounter::new(self.concurrency + 1);

        if let Some(pool) = &self.thread_pool {
            pool.in_place_scope(|scope| {
                let (main_ealloc_shard, worker_ealloc_shards) = ealloc_shards
                    .split_last_mut()
                    .expect("ealloc_shards.len() == self.concurrency + 1");
                debug_assert_eq!(worker_ealloc_shards.len(), self.concurrency);
                let (main_offline_shard, worker_offline_shards) = self
                    .offline_buffer
                    .shards
                    .split_last_mut()
                    .expect("offline shards.len() == self.concurrency + 1");
                debug_assert_eq!(worker_ealloc_shards.len(), self.concurrency);

                for (worker_id, (ealloc_shard, offline_shard)) in worker_ealloc_shards
                    .iter_mut()
                    .zip(worker_offline_shards.iter_mut())
                    .enumerate()
                {
                    let deadlock_counter = &deadlock_counter;
                    scope.spawn(move |_| {
                        threaded_worker(
                            worker_id,
                            tracer,
                            context,
                            send,
                            ealloc_shard,
                            offline_shard,
                            deadlock_counter,
                        )
                    });
                }

                main_worker(
                    tracer,
                    context,
                    send,
                    &mut unsend,
                    false,
                    main_ealloc_shard,
                    main_offline_shard,
                    &deadlock_counter,
                )
            });
        } else {
            main_worker(
                tracer,
                context,
                send,
                &mut unsend,
                true,
                ealloc_shards.get_mut(0).expect("concurrency = 0 in single-thread executor"),
                self.offline_buffer.shards.get_mut(0).expect("incorrect shard count"),
                &deadlock_counter,
            );
        }

        #[cfg(debug_assertions)]
        {
            use super::WakeupState;

            for (node, state) in &planner.get_mut().wakeup_state {
                let is_complete = matches!(state, WakeupState::Completed);
                if !is_complete {
                    panic!("Node {:?} state is {:?} instead of complete", node, state)
                }
            }
        }

        // ealloc_shards contains clones of the ShardState Arc,
        // which causes panic when flush_deallocate() is called
        drop(ealloc_shards);

        let sync_system_refs = sync_state
            .send_systems
            .iter_mut()
            .map(|(name, mutex)| (name.as_str(), mutex.get_mut().as_mut().as_descriptor_mut()));
        let unsend_system_refs = unsend
            .state
            .unsend_systems
            .iter_mut()
            .map(|(name, boxed)| (name.as_str(), boxed.as_mut().as_descriptor_mut()));
        let all_system_refs: Vec<_> = sync_system_refs.chain(unsend_system_refs).collect();

        self.offline_buffer.drain_cycle(
            WorldMut {
                ealloc_map,
                components,
                sync_globals,
                unsync_globals: unsend.globals,
                rctrack,
            },
            all_system_refs,
        );

        // TODO parallelize this loop
        for (&arch, ealloc) in &mut ealloc_map.map {
            let flush_ealloc_context = tracer.start_flush_ealloc(arch);
            ealloc.flush();
            tracer.end_flush_ealloc(flush_ealloc_context, arch);
        }

        tracer.end_cycle(cycle_context);
    }
}

#[allow(clippy::too_many_arguments)] // FIXME
fn main_worker(
    tracer: &impl Tracer,
    context: Context<'_>,
    send: SendArgs<'_>,
    unsend: &mut UnsendArgs<'_>,
    poll_send: bool,
    ealloc_shard_map: &mut ealloc::ShardMap,
    offline_buffer: &mut offline::BufferShard,
    deadlock_counter: &DeadlockCounter,
) {
    let mut planner_guard = context.planner.lock();

    loop {
        let steal = planner_guard.steal_unsend(tracer, tracer::Thread::Main, context.topology);
        match steal {
            StealResult::CycleComplete => return,
            StealResult::Pending if poll_send => {
                match planner_guard.steal_send(tracer, tracer::Thread::Main, context.topology) {
                    StealResult::CycleComplete => return,
                    StealResult::Pending => {
                        deadlock_counter.start_wait();
                        context.condvar.wait(&mut planner_guard);
                    }
                    StealResult::Ready(index) => {
                        MutexGuard::unlocked(&mut planner_guard, || {
                            let (debug_name, system) = send.state.get_send_system(index);

                            {
                                let mut system = system
                                    .try_lock()
                                    .expect("system should only be scheduled to one worker");
                                let run_context = tracer.start_run_sendable(
                                    tracer::Thread::Main,
                                    Node::SendSystem(index),
                                    debug_name,
                                    &mut **system,
                                );
                                system.run(
                                    send.globals,
                                    send.components,
                                    ealloc_shard_map,
                                    offline_buffer,
                                );
                                tracer.end_run_sendable(
                                    run_context,
                                    tracer::Thread::Main,
                                    Node::SendSystem(index),
                                    debug_name,
                                    &mut **system,
                                );
                            }
                        });

                        planner_guard.complete(
                            tracer,
                            Node::SendSystem(index),
                            context.topology,
                            context.condvar,
                            deadlock_counter,
                        );
                    }
                }
            }
            StealResult::Pending => {
                deadlock_counter.start_wait();
                context.condvar.wait(&mut planner_guard);
            }
            StealResult::Ready(index) => {
                MutexGuard::unlocked(&mut planner_guard, || {
                    let (debug_name, system) = unsend.state.get_unsend_system_mut(index);

                    let run_context = tracer.start_run_unsendable(
                        tracer::Thread::Main,
                        Node::UnsendSystem(index),
                        debug_name,
                        &mut *system,
                    );
                    system.run(
                        send.globals,
                        unsend.globals,
                        send.components,
                        ealloc_shard_map,
                        offline_buffer,
                    );
                    tracer.end_run_unsendable(
                        run_context,
                        tracer::Thread::Main,
                        Node::UnsendSystem(index),
                        debug_name,
                        &mut *system,
                    );
                });

                planner_guard.complete(
                    tracer,
                    Node::UnsendSystem(index),
                    context.topology,
                    context.condvar,
                    deadlock_counter,
                );
            }
        }
    }
}

fn threaded_worker(
    id: usize,
    tracer: &impl Tracer,
    context: Context<'_>,
    send: SendArgs<'_>,
    ealloc_shard_map: &mut ealloc::ShardMap,
    offline_buffer: &mut offline::BufferShard,
    deadlock_counter: &DeadlockCounter,
) {
    let thread = tracer::Thread::Worker(id);

    let mut planner_guard = context.planner.lock();

    loop {
        match planner_guard.steal_send(tracer, thread, context.topology) {
            StealResult::CycleComplete => return,
            StealResult::Pending => {
                deadlock_counter.start_wait();
                context.condvar.wait(&mut planner_guard);
            }
            StealResult::Ready(index) => {
                MutexGuard::unlocked(&mut planner_guard, || {
                    let (debug_name, system) = send.state.get_send_system(index);

                    {
                        let mut system = system
                            .try_lock()
                            .expect("system should only be scheduled to one worker");
                        let run_context = tracer.start_run_sendable(
                            thread,
                            Node::SendSystem(index),
                            debug_name,
                            &mut **system,
                        );
                        system.run(send.globals, send.components, ealloc_shard_map, offline_buffer);
                        tracer.end_run_sendable(
                            run_context,
                            thread,
                            Node::SendSystem(index),
                            debug_name,
                            &mut **system,
                        );
                    }
                });

                planner_guard.complete(
                    tracer,
                    Node::SendSystem(index),
                    context.topology,
                    context.condvar,
                    deadlock_counter,
                );
            }
        }
    }
}

#[cfg(debug_assertions)]
mod deadlock_counter {
    use std::sync::atomic::{self, AtomicUsize};

    pub(crate) struct DeadlockCounter(AtomicUsize);

    impl DeadlockCounter {
        pub(crate) fn new(concurrency: usize) -> Self { Self(AtomicUsize::new(concurrency)) }

        pub(crate) fn start_wait(&self) {
            let cnt = self.0.fetch_sub(1, atomic::Ordering::SeqCst);
            if cnt == 1 {
                panic!("Deadlock detected, all workers and main are waiting for tasks");
            }
        }

        pub(crate) fn end_wait(&self, count: usize) {
            self.0.fetch_add(count, atomic::Ordering::SeqCst);
        }
    }
}

#[cfg(not(debug_assertions))]
mod deadlock_counter {
    pub(crate) struct DeadlockCounter;

    impl DeadlockCounter {
        pub(crate) fn new(_concurrency: usize) -> Self { Self }
        pub(crate) fn start_wait(&self) {}
        pub(crate) fn end_wait(&self, _count: usize) {}
    }
}

pub(crate) use deadlock_counter::DeadlockCounter;

#[derive(Clone, Copy)]
struct Context<'t> {
    topology: &'t Topology,
    planner:  &'t Mutex<Planner>,
    condvar:  &'t Condvar,
}

#[derive(Clone, Copy)]
struct SendArgs<'t> {
    state:      &'t SyncState,
    components: &'t world::Components,
    globals:    &'t world::SyncGlobals,
}
