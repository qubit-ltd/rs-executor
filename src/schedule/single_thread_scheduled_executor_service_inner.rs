// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_atomic::Atomic;
use qubit_lock::ParkingLotMonitor;

use crate::service::{ExecutorServiceLifecycle, StopReport};

use super::single_thread_scheduled_executor_service_state::SingleThreadScheduledExecutorServiceState;

/// Shared state for the single-thread scheduled executor service.
pub(crate) struct SingleThreadScheduledExecutorServiceInner {
    /// Mutable lifecycle and heap state.
    pub(crate) state: ParkingLotMonitor<SingleThreadScheduledExecutorServiceState>,
}

impl SingleThreadScheduledExecutorServiceInner {
    /// Creates an empty scheduled service state.
    ///
    /// # Returns
    ///
    /// Shared scheduler state before its worker thread starts.
    pub(crate) fn new() -> Self {
        Self {
            state: ParkingLotMonitor::new(SingleThreadScheduledExecutorServiceState::new()),
        }
    }

    /// Returns the queued scheduled task count.
    ///
    /// # Returns
    ///
    /// Number of accepted tasks that have not started or been cancelled.
    #[inline]
    pub(crate) fn queued_count(&self) -> usize {
        self.state
            .with_read(SingleThreadScheduledExecutorServiceState::queued_count)
    }

    /// Returns the currently running task count.
    ///
    /// # Returns
    ///
    /// `1` when the scheduler thread is running a task, otherwise `0`.
    #[inline]
    pub(crate) fn running_count(&self) -> usize {
        self.state
            .with_read(SingleThreadScheduledExecutorServiceState::running_count)
    }

    /// Publishes cancellation for a queued task.
    ///
    /// # Parameters
    ///
    /// * `cancellation_marker` - Marker observed by the scheduler heap.
    ///
    /// # Panics
    ///
    /// Panics if no queued task is recorded or the cancellation count
    /// overflows.
    pub(crate) fn finish_queued_cancellation(&self, cancellation_marker: &Atomic<bool>) {
        self.state.with_write_notify_all(|state| {
            state.cancel_queued_task();
            cancellation_marker.store(true);
        });
    }

    /// Records completion for the currently running task.
    ///
    /// # Panics
    ///
    /// Panics if no running task is recorded or the completed task count
    /// overflows.
    pub(crate) fn finish_running_task(&self) {
        self.state
            .with_write_notify_all(SingleThreadScheduledExecutorServiceState::finish_running_task);
    }

    /// Requests graceful shutdown.
    pub(crate) fn shutdown(&self) {
        self.state.with_write_notify_all(|state| {
            if state.lifecycle == ExecutorServiceLifecycle::Running {
                state.lifecycle = ExecutorServiceLifecycle::ShuttingDown;
            }
        });
    }

    /// Requests immediate shutdown and cancels queued scheduled tasks.
    ///
    /// # Returns
    ///
    /// Count-based stop report.
    pub(crate) fn stop(&self) -> StopReport {
        let mut state = self.state.lock();
        state.lifecycle = ExecutorServiceLifecycle::Stopping;
        let queued = state.queued_count();
        let mut cancelled = 0;
        while let Some(task) = state.tasks.pop() {
            if task.entry.cancel() {
                state.cancel_queued_task();
                cancelled += 1;
            }
        }
        let running = state.running_count();
        self.state.notify_all();
        StopReport::new(queued, running, cancelled)
    }

    /// Returns whether shutdown has started.
    ///
    /// # Returns
    ///
    /// `true` if new scheduled tasks are rejected.
    pub(crate) fn is_not_running(&self) -> bool {
        self.state
            .with_read(|state| state.lifecycle != ExecutorServiceLifecycle::Running)
    }

    /// Returns the current lifecycle state.
    ///
    /// # Returns
    ///
    /// [`ExecutorServiceLifecycle::Terminated`] after the worker has exited,
    /// otherwise the stored lifecycle state.
    pub(crate) fn lifecycle(&self) -> ExecutorServiceLifecycle {
        self.state.with_read(|state| {
            if state.terminated {
                ExecutorServiceLifecycle::Terminated
            } else {
                state.lifecycle
            }
        })
    }

    /// Returns whether the scheduler thread has exited.
    ///
    /// # Returns
    ///
    /// `true` after shutdown and scheduler termination.
    pub(crate) fn is_terminated(&self) -> bool {
        self.state.with_read(|state| state.terminated)
    }

    /// Waits until the scheduler thread exits.
    pub(crate) fn wait_for_termination(&self) {
        self.state.wait_until(|state| state.terminated, |_| ());
    }

    /// Marks the scheduler thread as terminated.
    pub(crate) fn terminate(&self, state: &mut SingleThreadScheduledExecutorServiceState) {
        state.terminated = true;
        self.state.notify_all();
    }
}

impl Default for SingleThreadScheduledExecutorServiceInner {
    /// Creates an empty scheduled service state.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
