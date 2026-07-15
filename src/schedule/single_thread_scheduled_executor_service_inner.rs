// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_atomic::AtomicCount;
use qubit_lock::ParkingLotMonitor;

use crate::service::{
    ExecutorServiceLifecycle,
    StopReport,
};

use super::single_thread_scheduled_executor_service_state::SingleThreadScheduledExecutorServiceState;

/// Shared state for the single-thread scheduled executor service.
pub(crate) struct SingleThreadScheduledExecutorServiceInner {
    /// Mutable lifecycle and heap state.
    pub(crate) state:
        ParkingLotMonitor<SingleThreadScheduledExecutorServiceState>,
    /// Number of accepted tasks still waiting for their scheduled start.
    queued_task_count: AtomicCount,
    /// Number of tasks currently executing on the scheduler thread.
    running_task_count: AtomicCount,
    /// Number of tasks that ran to completion.
    completed_task_count: AtomicCount,
    /// Number of scheduled tasks cancelled before execution.
    cancelled_task_count: AtomicCount,
}

impl SingleThreadScheduledExecutorServiceInner {
    /// Creates an empty scheduled service state.
    ///
    /// # Returns
    ///
    /// Shared scheduler state before its worker thread starts.
    pub(crate) fn new() -> Self {
        Self {
            state: ParkingLotMonitor::new(
                SingleThreadScheduledExecutorServiceState::new(),
            ),
            queued_task_count: AtomicCount::zero(),
            running_task_count: AtomicCount::zero(),
            completed_task_count: AtomicCount::zero(),
            cancelled_task_count: AtomicCount::zero(),
        }
    }

    /// Returns the queued scheduled task count.
    ///
    /// # Returns
    ///
    /// Number of accepted tasks that have not started or been cancelled.
    #[inline]
    pub(crate) fn queued_count(&self) -> usize {
        self.queued_task_count.get()
    }

    /// Returns the currently running task count.
    ///
    /// # Returns
    ///
    /// `1` when the scheduler thread is running a task, otherwise `0`.
    #[inline]
    pub(crate) fn running_count(&self) -> usize {
        self.running_task_count.get()
    }

    /// Records that a queued task has been accepted.
    ///
    /// # Panics
    ///
    /// Panics if the queued task count overflows.
    #[inline]
    pub(crate) fn add_queued_task(&self) {
        self.queued_task_count.inc();
    }

    /// Records that a queued task was cancelled before start.
    ///
    /// # Panics
    ///
    /// Panics if no queued task is recorded or the cancellation count
    /// overflows.
    pub(crate) fn finish_queued_cancellation(&self) {
        self.queued_task_count.dec();
        self.cancelled_task_count.inc();
        self.state.notify_all();
    }

    /// Records that a queued task has become running.
    ///
    /// # Panics
    ///
    /// Panics if no queued task is recorded or the running task count
    /// overflows.
    pub(crate) fn start_task(&self) {
        self.queued_task_count.dec();
        self.running_task_count.inc();
    }

    /// Records completion for the currently running task.
    ///
    /// # Panics
    ///
    /// Panics if no running task is recorded or the completed task count
    /// overflows.
    pub(crate) fn finish_running_task(&self) {
        self.running_task_count.dec();
        self.completed_task_count.inc();
        self.state.notify_all();
    }

    /// Requests graceful shutdown.
    pub(crate) fn shutdown(&self) {
        self.state.with_write(|state| {
            if state.lifecycle == ExecutorServiceLifecycle::Running {
                state.lifecycle = ExecutorServiceLifecycle::ShuttingDown;
            }
        });
        self.state.notify_all();
    }

    /// Requests immediate shutdown and cancels queued scheduled tasks.
    ///
    /// # Returns
    ///
    /// Count-based stop report.
    pub(crate) fn stop(&self) -> StopReport {
        let mut state = self.state.lock();
        state.lifecycle = ExecutorServiceLifecycle::Stopping;
        let queued = self.queued_count();
        let mut cancelled = 0;
        while let Some(task) = state.tasks.pop() {
            if task.entry.cancel() {
                self.finish_queued_cancellation();
                cancelled += 1;
            }
        }
        let running = self.running_count();
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
    pub(crate) fn terminate(
        &self,
        state: &mut SingleThreadScheduledExecutorServiceState,
    ) {
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
