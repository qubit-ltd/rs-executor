// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::panic::resume_unwind;
use std::time::Duration;
use std::time::Instant;

use qubit_collections::map::ordered_index_map::OwnedEntry;
use qubit_lock::ParkingLotMonitor;
use qubit_lock::WaitTimeoutResult;

use super::internal::StopDrainGuard;
use super::scheduled_task_entry::ScheduledTaskEntry;
use super::scheduler_state::SchedulerState;
use crate::hook::TaskId;
use crate::internal::discard_panic_payload;
use crate::service::ExecutorServiceLifecycle;
use crate::service::StopReport;
use crate::service::SubmissionError;

/// Detached scheduled entries drained during an immediate stop.
type ScheduledTaskEntries = Vec<OwnedEntry<TaskId, Instant, Box<dyn ScheduledTaskEntry>>>;

/// Shared coordinator for a single scheduled worker.
pub(crate) struct SchedulerCore {
    /// Mutable scheduler state and its condition variable.
    pub(crate) state: ParkingLotMonitor<SchedulerState>,
}

impl SchedulerCore {
    /// Creates an empty scheduler coordinator.
    ///
    /// # Returns
    ///
    /// A coordinator with no queued tasks and a running lifecycle.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            state: ParkingLotMonitor::new(SchedulerState::new()),
        }
    }

    /// Creates an empty scheduler coordinator with an optional queue limit.
    ///
    /// # Parameters
    ///
    /// * `queue_capacity` - Maximum number of queued entries, or `None` for no
    ///   queue limit.
    ///
    /// # Returns
    ///
    /// A coordinator with no queued tasks and a running lifecycle.
    #[must_use]
    pub(crate) fn with_queue_capacity(queue_capacity: Option<usize>) -> Self {
        Self {
            state: ParkingLotMonitor::new(SchedulerState::with_queue_capacity(queue_capacity)),
        }
    }

    /// Returns the queued entry count.
    ///
    /// # Returns
    ///
    /// The number of entries waiting for their deadlines.
    #[must_use]
    #[inline]
    pub(crate) fn queued_count(&self) -> usize {
        self.state.with_read(|state| state.tasks.len())
    }

    /// Returns the active worker count.
    ///
    /// # Returns
    ///
    /// `1` when the scheduled worker owns an entry, otherwise `0`.
    #[must_use]
    #[inline]
    pub(crate) fn running_count(&self) -> usize {
        self.state.with_read(|state| usize::from(state.worker_active))
    }

    /// Returns whether shutdown has started.
    ///
    /// # Returns
    ///
    /// `true` after the lifecycle leaves `Running`.
    #[must_use]
    #[inline]
    pub(crate) fn is_not_running(&self) -> bool {
        self.state
            .with_read(|state| state.lifecycle != ExecutorServiceLifecycle::Running)
    }

    /// Returns the current lifecycle.
    ///
    /// # Returns
    ///
    /// The requested lifecycle, reported as `Terminated` after termination.
    #[must_use]
    #[inline]
    pub(crate) fn lifecycle(&self) -> ExecutorServiceLifecycle {
        self.state.with_read(|state| {
            if state.terminated {
                ExecutorServiceLifecycle::Terminated
            } else {
                state.lifecycle
            }
        })
    }

    /// Returns whether the worker has exited.
    ///
    /// # Returns
    ///
    /// `true` after the worker publishes termination.
    #[must_use]
    #[inline]
    pub(crate) fn is_terminated(&self) -> bool {
        self.state.with_read(|state| state.terminated)
    }

    /// Waits for worker termination.
    ///
    /// This call blocks until the worker or stop path publishes termination.
    pub(crate) fn wait_for_termination(&self) {
        self.state.wait_until_ready(|state| state.terminated);
    }

    /// Waits for worker termination for at most `timeout`.
    ///
    /// # Parameters
    ///
    /// * `timeout` - Maximum duration to wait, measured from this call.
    ///
    /// # Returns
    ///
    /// `true` if termination was observed before the timeout expired.
    ///
    /// # Panics
    ///
    /// Panics if the monitor reports an error while waiting.
    #[must_use]
    pub(crate) fn wait_for_termination_timeout(&self, timeout: Duration) -> bool {
        self.wait_for_termination_timeout_with_waiter(timeout, |slice| {
            match self
                .state
                .wait_until_ready_with_total_timeout(slice, |state| state.terminated)
            {
                Ok(WaitTimeoutResult::Ready(())) => true,
                Ok(WaitTimeoutResult::TimedOut) => false,
                Err(error) => panic!("scheduler termination wait failed: {error}"),
            }
        })
    }

    /// Waits for termination, passing bounded remaining-time slices to
    /// `waiter`.
    ///
    /// A `true` waiter result reports termination; `false` continues waiting.
    ///
    /// # Type Parameters
    ///
    /// * `F` - Wait operation receiving the bounded remaining-time slice.
    ///
    /// # Parameters
    ///
    /// * `timeout` - Maximum duration to wait for termination.
    /// * `waiter` - Operation that waits for one slice and reports termination.
    fn wait_for_termination_timeout_with_waiter<F>(&self, timeout: Duration, mut waiter: F) -> bool
    where
        F: FnMut(Duration) -> bool,
    {
        let started = Instant::now();
        loop {
            if self.is_terminated() {
                return true;
            }
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return false;
            }
            if waiter(remaining.min(Duration::from_secs(3600))) {
                return true;
            }
        }
    }

    /// Inserts and accepts an entry while the service is running.
    ///
    /// # Parameters
    ///
    /// * `task_id` - Unique identifier used to address the scheduled task.
    /// * `deadline` - Instant at which the worker may start the task.
    /// * `entry` - Type-erased task entry to accept and queue.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::Shutdown`] if shutdown has started.
    ///
    /// # Panics
    ///
    /// Panics if `task_id` is already queued.
    pub(crate) fn schedule(
        &self,
        task_id: TaskId,
        deadline: Instant,
        entry: Box<dyn ScheduledTaskEntry>,
    ) -> Result<(), SubmissionError> {
        let mut state = self.state.lock();
        if state.lifecycle != ExecutorServiceLifecycle::Running {
            drop(state);
            return Err(SubmissionError::Shutdown);
        }
        if state.queue_capacity.is_some_and(|limit| state.tasks.len() >= limit) {
            drop(state);
            return Err(SubmissionError::Saturated);
        }
        if state.tasks.try_insert(task_id, deadline, entry).is_err() {
            panic!("task identifiers must be unique while scheduled");
        }
        let entry = state
            .tasks
            .get(&task_id)
            .expect("inserted scheduled task must remain addressable");
        entry.accept();
        state.notify_all();
        Ok(())
    }

    /// Removes a queued entry after its handle publishes cancellation.
    ///
    /// # Parameters
    ///
    /// * `task_id` - Identifier of the entry to remove.
    pub(crate) fn cancel_queued_task(&self, task_id: TaskId) {
        let removed = self.state.with_write_notify_all(|state| state.tasks.remove(&task_id));
        drop(removed);
    }

    /// Requests graceful shutdown.
    ///
    /// New entries are rejected while queued entries remain available to the
    /// worker until they complete or are stopped.
    pub(crate) fn shutdown(&self) {
        self.state.with_write_notify_all(|state| {
            if state.lifecycle == ExecutorServiceLifecycle::Running {
                state.lifecycle = ExecutorServiceLifecycle::ShuttingDown;
            }
        });
    }

    /// Requests immediate shutdown and cancels detached queued entries.
    ///
    /// Cancellation callbacks run after the scheduler lock is released.
    /// If an entry panics during ordinary unwinding, remaining entries are
    /// still cancelled and the first panic is resumed after drain state is
    /// restored. This does not recover from aborting panics.
    ///
    /// # Returns
    ///
    /// Counts observed at the start of the stop request and the number of
    /// entries whose cancellation was published.
    #[must_use]
    pub(crate) fn stop(&self) -> StopReport {
        let (entries, queued, running, owns_drain): (ScheduledTaskEntries, usize, usize, bool) =
            self.state.with_write_notify_all(|state| {
                if state.lifecycle == ExecutorServiceLifecycle::Terminated {
                    return (Vec::new(), 0, 0, false);
                }
                state.lifecycle = ExecutorServiceLifecycle::Stopping;
                let queued = state.tasks.len();
                let running = usize::from(state.worker_active);
                let mut entries = Vec::with_capacity(queued);
                while let Some(entry) = state.tasks.pop_first() {
                    entries.push(entry);
                }
                let owns_drain = !entries.is_empty();
                if owns_drain {
                    state.stop_draining = true;
                }
                (entries, queued, running, owns_drain)
            });
        let entries: ScheduledTaskEntries = entries;
        let drain_guard = owns_drain.then(|| StopDrainGuard::new(self));
        let mut first_panic = None;
        let mut cancelled = 0;
        for entry in entries {
            match catch_unwind(AssertUnwindSafe(|| entry.into_value().cancel())) {
                Ok(was_cancelled) => cancelled += usize::from(was_cancelled),
                Err(payload) => {
                    if first_panic.is_none() {
                        first_panic = Some(payload);
                    } else {
                        discard_panic_payload(payload);
                    }
                }
            }
        }
        drop(drain_guard);
        self.state.with_write_notify_all(|state| {
            if state.can_terminate() {
                state.terminated = true;
            }
        });
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
        StopReport::new(queued, running, cancelled)
    }
}

impl Default for SchedulerCore {
    /// Creates an empty scheduler coordinator.
    ///
    /// # Returns
    ///
    /// A coordinator with no queued tasks and a running lifecycle.
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use std::time::Instant;

    use super::super::completable_scheduled_task::CompletableScheduledTask;
    use super::super::scheduled_task_entry::ScheduledTaskEntry;
    use super::super::scheduled_task_entry::StartedScheduledTask;
    use super::SchedulerCore;
    use crate::TaskExecutionError;
    use crate::hook::TaskId;
    use crate::task::TryGet;
    use crate::task::spi::TaskEndpointPair;

    struct PanicOnCancelEntry;

    struct PanicOnDrop;

    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("stop payload drop panic");
        }
    }

    impl ScheduledTaskEntry for PanicOnCancelEntry {
        fn accept(&self) {}

        fn start(self: Box<Self>) -> Option<StartedScheduledTask> {
            None
        }

        fn cancel(self: Box<Self>) -> bool {
            std::panic::panic_any(PanicOnDrop);
        }
    }

    struct BlockingCancelEntry {
        entered: Option<mpsc::Sender<()>>,
        release: Option<mpsc::Receiver<()>>,
    }

    impl ScheduledTaskEntry for BlockingCancelEntry {
        fn accept(&self) {}

        fn start(self: Box<Self>) -> Option<StartedScheduledTask> {
            None
        }

        fn cancel(mut self: Box<Self>) -> bool {
            self.entered
                .take()
                .expect("cancel entry signal")
                .send(())
                .expect("stop test receiver");
            self.release
                .take()
                .expect("cancel release receiver")
                .recv_timeout(Duration::from_secs(5))
                .expect("stop test release should arrive within five seconds");
            true
        }
    }

    #[test]
    fn test_concurrent_stop_waits_for_first_drain() {
        crate::internal::test_watchdog_tests::run_test(
            "schedule::scheduler_core::tests::test_concurrent_stop_waits_for_first_drain",
            "scheduler-core-concurrent-stop-drain",
            Duration::from_secs(15),
            || {
                let core = Arc::new(SchedulerCore::new());
                let (entered_tx, entered_rx) = mpsc::channel();
                let (release_tx, release_rx) = mpsc::channel();
                core.schedule(
                    TaskId::new(1),
                    Instant::now() + Duration::from_secs(60),
                    Box::new(BlockingCancelEntry {
                        entered: Some(entered_tx),
                        release: Some(release_rx),
                    }),
                )
                .expect("entry should schedule");

                let stop_core = Arc::clone(&core);
                let first_stop = thread::spawn(move || stop_core.stop());
                entered_rx
                    .recv_timeout(Duration::from_secs(5))
                    .expect("first stop should enter cancellation within five seconds");
                let second = core.stop();
                let terminated_early = core.is_terminated();
                release_tx.send(()).expect("first stop should be released");
                let first = first_stop.join().expect("first stop thread");

                assert!(!terminated_early);
                assert!(core.is_terminated());
                assert_eq!(first.cancelled, 1);
                assert_eq!(second.queued, 0);
            },
        )
        .expect("watchdog should observe concurrent stop test finish");
    }

    #[test]
    fn test_max_termination_timeout_dispatches_bounded_wait() {
        crate::internal::test_watchdog_tests::run_test(
            "schedule::scheduler_core::tests::test_max_termination_timeout_dispatches_bounded_wait",
            "scheduler-core-max-timeout-dispatch",
            Duration::from_secs(15),
            || {
                let core = Arc::new(SchedulerCore::new());
                let (entered_tx, entered_rx) = mpsc::channel();
                let (finished_tx, finished_rx) = mpsc::channel();
                let worker_core = Arc::clone(&core);
                let coordinator = thread::spawn(move || {
                    entered_rx
                        .recv_timeout(Duration::from_secs(5))
                        .expect("waiter should be entered");
                    worker_core.shutdown();
                    worker_core.state.with_write_notify_all(|state| {
                        assert!(state.can_terminate());
                        state.terminated = true;
                    });
                    finished_tx.send(()).expect("wait observer exists");
                });

                assert!(core.wait_for_termination_timeout_with_waiter(Duration::MAX, |slice| {
                    assert_eq!(slice, Duration::from_secs(3600));
                    assert!(!core.is_terminated());
                    entered_tx.send(()).expect("termination coordinator exists");
                    finished_rx
                        .recv_timeout(Duration::from_secs(5))
                        .expect("coordinator should publish termination");
                    assert!(core.is_terminated());
                    true
                }));
                coordinator.join().expect("termination coordinator thread");
            },
        )
        .expect("watchdog should observe max-timeout dispatch test finish");
    }

    #[test]
    fn test_zero_termination_timeout_does_not_wait() {
        let core = SchedulerCore::new();
        assert!(!core.wait_for_termination_timeout_with_waiter(Duration::ZERO, |_| { panic!("must not wait") }));
    }

    #[test]
    fn test_terminated_core_does_not_wait() {
        let core = SchedulerCore::new();
        core.shutdown();
        core.state.with_write_notify_all(|state| {
            assert!(state.can_terminate());
            state.terminated = true;
        });
        assert!(core.wait_for_termination_timeout_with_waiter(Duration::MAX, |_| { panic!("must not wait") }));
    }

    #[test]
    fn test_stop_discards_later_cancel_panics_and_drains_all_entries() {
        let core = SchedulerCore::new();
        let deadline = Instant::now() + Duration::from_secs(60);
        core.schedule(TaskId::new(10), deadline, Box::new(PanicOnCancelEntry))
            .expect("first panicking entry should schedule");
        core.schedule(TaskId::new(11), deadline, Box::new(PanicOnCancelEntry))
            .expect("second panicking entry should schedule");
        let (handle, slot) = TaskEndpointPair::<usize, ()>::new().into_parts();
        core.schedule(
            TaskId::new(12),
            deadline,
            Box::new(CompletableScheduledTask::new(|| Ok::<usize, ()>(1), slot)),
        )
        .expect("following entry should schedule");

        let stop = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| core.stop()));
        let Err(payload) = stop else {
            panic!("first cancellation panic should be resumed");
        };
        crate::internal::discard_panic_payload(payload);

        assert!(matches!(
            handle.try_get(),
            TryGet::Ready(Err(TaskExecutionError::Cancelled))
        ));
        assert_eq!(core.queued_count(), 0);
        assert!(core.is_terminated());
    }
}
