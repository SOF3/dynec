use parking_lot::{Condvar, Mutex, MutexGuard};

use super::planner::StealResult;
use super::{Node, Planner, SyncState, Topology, UnsyncState, WakeupState};
use crate::world::{Components, SendGlobals, UnsendGlobals};

pub(in crate::world::scheduler) struct Executor {
    thread_pool: Option<rayon::ThreadPool>,
    concurrency: usize,
}

impl Executor {
    /// Builds a new executor with the given `concurrency`.
    ///
    /// Note that `concurrency` only specifies the number of worker threads.
    /// The main thread is not considered a worker thread.
    /// Therefore, it is valid to set a concurrency of 0,
    /// especially in environments where threading is not supported.
    pub(in crate::world::scheduler) fn new(concurrency: usize) -> Self {
        Self {
            thread_pool: (concurrency > 0).then(|| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(concurrency)
                    .thread_name(|i| format!("dynec executor #{}", i))
                    .build()
                    .expect("Failed to create thread pool")
            }),
            concurrency,
        }
    }

    pub(in crate::world::scheduler) fn execute_full_cycle(
        &self,
        topology: &Topology,
        planner: &mut Mutex<Planner>,
        sync_state: &SyncState,
        unsync_state: &mut UnsyncState,
        components: &Components,
        send_globals: &SendGlobals,
        unsend_globals: &UnsendGlobals,
    ) {
        let condvar = Condvar::new();

        planner.get_mut().clone_from(topology.initial_planner());

        if let Some(pool) = &self.thread_pool {
            pool.in_place_scope(|scope| {
                for _ in 0..self.concurrency {
                    scope.spawn(|_| {
                        threaded_worker(
                            topology,
                            &*planner,
                            &condvar,
                            sync_state,
                            components,
                            send_globals,
                        )
                    });
                }

                let poll_send = self.concurrency == 0;
                main_worker(
                    topology,
                    &*planner,
                    &condvar,
                    sync_state,
                    unsync_state,
                    components,
                    send_globals,
                    unsend_globals,
                    poll_send,
                )
            });
        }

        debug_assert!(planner
            .get_mut()
            .wakeup_state
            .values()
            .all(|state| matches!(state, WakeupState::Completed)));
    }
}

fn main_worker(
    topology: &Topology,
    planner: &Mutex<Planner>,
    condvar: &Condvar,
    sync_state: &SyncState,
    unsync_state: &mut UnsyncState,
    components: &Components,
    send_globals: &SendGlobals,
    unsend_globals: &UnsendGlobals,
    poll_send: bool,
) {
    let mut planner_guard = planner.lock();

    loop {
        match planner_guard.steal_unsend(topology) {
            StealResult::CycleComplete => return,
            StealResult::Pending if poll_send => match planner_guard.steal_send(topology) {
                StealResult::CycleComplete => return,
                StealResult::Pending => condvar.wait(&mut planner_guard),
                StealResult::Ready(index) => {
                    MutexGuard::unlocked(&mut planner_guard, || {
                        let system = sync_state.get_send_system(index);

                        {
                            let mut system = system
                                .try_lock()
                                .expect("system should only be scheduled to one worker");
                            system.run(send_globals, components);
                        }
                    });

                    planner_guard.complete(Node::SendSystem(index), topology, condvar);
                }
            },
            StealResult::Pending => condvar.wait(&mut planner_guard),
            StealResult::Ready(index) => {
                MutexGuard::unlocked(&mut planner_guard, || {
                    let system = unsync_state.get_unsend_system_mut(index);

                    {
                        system.run(send_globals, unsend_globals, components);
                    }
                });

                planner_guard.complete(Node::UnsendSystem(index), topology, condvar);
            }
        }
    }
}

fn threaded_worker(
    topology: &Topology,
    planner: &Mutex<Planner>,
    condvar: &Condvar,
    sync_state: &SyncState,
    components: &Components,
    send_globals: &SendGlobals,
) {
    let mut planner_guard = planner.lock();

    loop {
        match planner_guard.steal_send(topology) {
            StealResult::CycleComplete => return,
            StealResult::Pending => condvar.wait(&mut planner_guard),
            StealResult::Ready(index) => {
                MutexGuard::unlocked(&mut planner_guard, || {
                    let system = sync_state.get_send_system(index);

                    {
                        let mut system = system
                            .try_lock()
                            .expect("system should only be scheduled to one worker");
                        system.run(send_globals, components);
                    }
                });

                planner_guard.complete(Node::SendSystem(index), topology, condvar);
            }
        }
    }
}
