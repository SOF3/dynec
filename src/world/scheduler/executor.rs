use parking_lot::{Condvar, Mutex, MutexGuard};

use super::planner::StealResult;
use super::{Node, Planner, SendArgs, Topology, UnsendArgs, WakeupState};

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
        send: SendArgs<'_>,
        unsend: UnsendArgs<'_>,
    ) {
        let condvar = Condvar::new();

        planner.get_mut().clone_from(topology.initial_planner());

        let context = Context { topology, planner, condvar: &condvar };

        if let Some(pool) = &self.thread_pool {
            pool.in_place_scope(|scope| {
                for _ in 0..self.concurrency {
                    scope.spawn(|_| threaded_worker(context, send));
                }

                let poll_send = self.concurrency == 0;
                main_worker(context, send, unsend, poll_send)
            });
        }

        debug_assert!(planner
            .get_mut()
            .wakeup_state
            .values()
            .all(|state| matches!(state, WakeupState::Completed)));
    }
}

fn main_worker(context: Context<'_>, send: SendArgs<'_>, unsend: UnsendArgs<'_>, poll_send: bool) {
    let mut planner_guard = context.planner.lock();

    loop {
        match planner_guard.steal_unsend(context.topology) {
            StealResult::CycleComplete => return,
            StealResult::Pending if poll_send => match planner_guard.steal_send(context.topology) {
                StealResult::CycleComplete => return,
                StealResult::Pending => context.condvar.wait(&mut planner_guard),
                StealResult::Ready(index) => {
                    MutexGuard::unlocked(&mut planner_guard, || {
                        let system = send.sync_state.get_send_system(index);

                        {
                            let mut system = system
                                .try_lock()
                                .expect("system should only be scheduled to one worker");
                            system.run(send.sync_globals, send.components);
                        }
                    });

                    planner_guard.complete(
                        Node::SendSystem(index),
                        context.topology,
                        context.condvar,
                    );
                }
            },
            StealResult::Pending => context.condvar.wait(&mut planner_guard),
            StealResult::Ready(index) => {
                MutexGuard::unlocked(&mut planner_guard, || {
                    let system = unsend.unsync_state.get_unsend_system_mut(index);

                    {
                        system.run(send.sync_globals, unsend.unsync_globals, send.components);
                    }
                });

                planner_guard.complete(
                    Node::UnsendSystem(index),
                    context.topology,
                    context.condvar,
                );
            }
        }
    }
}

fn threaded_worker(context: Context<'_>, send: SendArgs<'_>) {
    let mut planner_guard = context.planner.lock();

    loop {
        match planner_guard.steal_send(context.topology) {
            StealResult::CycleComplete => return,
            StealResult::Pending => context.condvar.wait(&mut planner_guard),
            StealResult::Ready(index) => {
                MutexGuard::unlocked(&mut planner_guard, || {
                    let system = send.sync_state.get_send_system(index);

                    {
                        let mut system = system
                            .try_lock()
                            .expect("system should only be scheduled to one worker");
                        system.run(send.sync_globals, send.components);
                    }
                });

                planner_guard.complete(Node::SendSystem(index), context.topology, context.condvar);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Context<'t> {
    topology: &'t Topology,
    planner:  &'t Mutex<Planner>,
    condvar:  &'t Condvar,
}
