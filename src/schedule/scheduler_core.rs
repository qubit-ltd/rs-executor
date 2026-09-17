// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::time::Duration;
use std::time::Instant;

use qubit_collections::map::ordered_index_map::OwnedEntry;
use qubit_lock::ParkingLotMonitor;
use qubit_lock::WaitTimeoutResult;

use super::scheduled_task_entry::ScheduledTaskEntry;
use super::scheduler_state::SchedulerState;
use crate::hook::TaskId;
use crate::service::ExecutorServiceLifecycle;
use crate::service::StopReport;

type ScheduledTaskEntries = Vec<OwnedEntry<TaskId, Instant, Box<dyn ScheduledTaskEntry>>>;

/// Shared coordinator for a single scheduled worker.
pub(crate) struct SchedulerCore {
    /// Mutable scheduler state and its condition variable.
    pub(crate) state: ParkingLotMonitor<SchedulerState>,
}

impl SchedulerCore {
    /// Creates an empty scheduler coordinator.
    pub(crate) fn new() -> Self {
        Self {
            state: ParkingLotMonitor::new(SchedulerState::new()),
        }
    }

    /// Returns the queued entry count.
    pub(crate) fn queued_count(&self) -> usize {
        self.state.with_read(|state| state.tasks.len())
    }

    /// Returns the active worker count.
    pub(crate) fn running_count(&self) -> usize {
        self.state.with_read(|state| usize::from(state.worker_active))
    }

    /// Inserts and accepts an entry while the service is running.
    pub(crate) fn schedule(
        &self,
        task_id: TaskId,
        deadline: Instant,
        entry: Box<dyn ScheduledTaskEntry>,
    ) -> Result<(), crate::service::SubmissionError> {
        let mut state = self.state.lock();
        if state.lifecycle != ExecutorServiceLifecycle::Running {
            return Err(crate::service::SubmissionError::Shutdown);
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
    pub(crate) fn cancel_queued_task(&self, task_id: TaskId) {
        let removed = self.state.with_write_notify_all(|state| state.tasks.remove(&task_id));
        drop(removed);
    }

    /// Requests graceful shutdown.
    pub(crate) fn shutdown(&self) {
        self.state.with_write_notify_all(|state| {
            if state.lifecycle == ExecutorServiceLifecycle::Running {
                state.lifecycle = ExecutorServiceLifecycle::ShuttingDown;
            }
        });
    }

    /// Requests immediate shutdown and cancels detached queued entries.
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
        let mut cancelled = 0;
        for entry in entries {
            cancelled += usize::from(entry.into_value().cancel());
        }
        self.state.with_write_notify_all(|state| {
            if owns_drain {
                state.stop_draining = false;
            }
            if state.can_terminate() {
                state.terminated = true;
            }
        });
        StopReport::new(queued, running, cancelled)
    }

    /// Returns whether shutdown has started.
    pub(crate) fn is_not_running(&self) -> bool {
        self.state
            .with_read(|state| state.lifecycle != ExecutorServiceLifecycle::Running)
    }

    /// Returns the current lifecycle.
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
    pub(crate) fn is_terminated(&self) -> bool {
        self.state.with_read(|state| state.terminated)
    }

    /// Waits for worker termination.
    pub(crate) fn wait_for_termination(&self) {
        self.state.wait_until_ready(|state| state.terminated);
    }

    /// Waits for worker termination for at most `timeout`.
    pub(crate) fn wait_for_termination_timeout(&self, timeout: Duration) -> bool {
        let started = Instant::now();
        loop {
            if self.is_terminated() {
                return true;
            }
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return false;
            }
            let slice = remaining.min(Duration::from_secs(3600));
            match self
                .state
                .wait_until_ready_with_total_timeout(slice, |state| state.terminated)
            {
                Ok(WaitTimeoutResult::Ready(())) => return true,
                Ok(WaitTimeoutResult::TimedOut) => {}
                Err(error) => panic!("scheduler termination wait failed: {error}"),
            }
        }
    }
}

impl Default for SchedulerCore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use super::super::scheduled_task_entry::ScheduledTaskEntry;
    use super::super::scheduled_task_entry::StartedScheduledTask;
    use super::SchedulerCore;
    use crate::hook::TaskId;

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
                .recv()
                .expect("stop test release");
            true
        }
    }

    #[test]
    fn test_concurrent_stop_waits_for_first_drain() {
        let core = std::sync::Arc::new(SchedulerCore::new());
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        core.schedule(
            TaskId::new(1),
            std::time::Instant::now() + Duration::from_secs(60),
            Box::new(BlockingCancelEntry {
                entered: Some(entered_tx),
                release: Some(release_rx),
            }),
        )
        .expect("entry should schedule");

        let stop_core = std::sync::Arc::clone(&core);
        let first_stop = thread::spawn(move || stop_core.stop());
        entered_rx.recv().expect("first stop should enter cancellation");
        let second = core.stop();
        let terminated_early = core.is_terminated();
        release_tx.send(()).expect("first stop should be released");
        let first = first_stop.join().expect("first stop thread");

        assert!(!terminated_early);
        assert!(core.is_terminated());
        assert_eq!(first.cancelled, 1);
        assert_eq!(second.queued, 0);
    }
}
