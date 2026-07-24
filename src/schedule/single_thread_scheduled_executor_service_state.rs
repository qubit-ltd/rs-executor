// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::collections::BinaryHeap;

use crate::service::ExecutorServiceLifecycle;

use super::scheduled_task::ScheduledTask;

/// Mutable scheduler state protected by the service monitor.
pub(crate) struct SingleThreadScheduledExecutorServiceState {
    /// Current lifecycle state.
    pub(crate) lifecycle: ExecutorServiceLifecycle,
    /// Deadline-ordered task heap.
    pub(crate) tasks: BinaryHeap<ScheduledTask>,
    /// Sequence used to keep stable order for identical deadlines.
    pub(crate) next_sequence: usize,
    /// Whether the scheduler thread has exited.
    pub(crate) terminated: bool,
    /// Number of accepted tasks waiting for their scheduled start.
    queued_task_count: usize,
    /// Number of tasks currently executing on the scheduler thread.
    running_task_count: usize,
    /// Number of tasks that ran to completion.
    completed_task_count: usize,
    /// Number of scheduled tasks cancelled before execution.
    cancelled_task_count: usize,
}

impl SingleThreadScheduledExecutorServiceState {
    /// Creates an empty running scheduler state.
    ///
    /// # Returns
    ///
    /// A running state with no scheduled tasks.
    pub(crate) fn new() -> Self {
        Self {
            lifecycle: ExecutorServiceLifecycle::Running,
            tasks: BinaryHeap::new(),
            next_sequence: 0,
            terminated: false,
            queued_task_count: 0,
            running_task_count: 0,
            completed_task_count: 0,
            cancelled_task_count: 0,
        }
    }

    /// Returns the queued scheduled task count.
    ///
    /// # Returns
    ///
    /// Number of accepted tasks that have not started or been cancelled.
    #[inline]
    pub(crate) const fn queued_count(&self) -> usize {
        self.queued_task_count
    }

    /// Returns the currently running task count.
    ///
    /// # Returns
    ///
    /// `1` when the scheduler thread is running a task, otherwise `0`.
    #[inline]
    pub(crate) const fn running_count(&self) -> usize {
        self.running_task_count
    }

    /// Records that a queued task has been accepted.
    ///
    /// # Panics
    ///
    /// Panics if the queued task count overflows.
    #[inline]
    pub(crate) fn accept_task(&mut self) {
        self.queued_task_count = self
            .queued_task_count
            .checked_add(1)
            .expect("scheduled service queued counter overflow");
    }

    /// Records that a queued task was cancelled before start.
    ///
    /// # Panics
    ///
    /// Panics if no queued task is recorded or the cancellation count
    /// overflows.
    pub(crate) fn cancel_queued_task(&mut self) {
        let queued_task_count = self
            .queued_task_count
            .checked_sub(1)
            .expect("scheduled service queued counter underflow");
        let cancelled_task_count = self
            .cancelled_task_count
            .checked_add(1)
            .expect("scheduled service cancelled counter overflow");
        self.queued_task_count = queued_task_count;
        self.cancelled_task_count = cancelled_task_count;
    }

    /// Records that a queued task has become running.
    ///
    /// # Panics
    ///
    /// Panics if no queued task is recorded or the running task count
    /// overflows.
    pub(crate) fn start_task(&mut self) {
        let queued_task_count = self
            .queued_task_count
            .checked_sub(1)
            .expect("scheduled service queued counter underflow");
        let running_task_count = self
            .running_task_count
            .checked_add(1)
            .expect("scheduled service running counter overflow");
        self.queued_task_count = queued_task_count;
        self.running_task_count = running_task_count;
    }

    /// Records completion for the currently running task.
    ///
    /// # Panics
    ///
    /// Panics if no running task is recorded or the completed task count
    /// overflows.
    pub(crate) fn finish_running_task(&mut self) {
        let running_task_count = self
            .running_task_count
            .checked_sub(1)
            .expect("scheduled service running counter underflow");
        let completed_task_count = self
            .completed_task_count
            .checked_add(1)
            .expect("scheduled service completed counter overflow");
        self.running_task_count = running_task_count;
        self.completed_task_count = completed_task_count;
    }
}
