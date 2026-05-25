/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::sync::Arc;

use oneshot::Sender;
use parking_lot::Mutex;
use qubit_atomic::Atomic;

use super::{
    TaskExecutionError,
    TaskResult,
    atomic_task_status::AtomicTaskStatus,
    task_status::TaskStatus,
};
use crate::hook::{
    TaskId,
    notify_finished,
    notify_started,
};

/// Shared completion endpoint state for one submitted task.
pub(crate) struct TaskState<R, E> {
    /// Identifier assigned to this task.
    pub(crate) task_id: TaskId,
    /// Atomic task status used for start, completion, and cancellation races.
    pub(crate) status: AtomicTaskStatus,
    /// Whether submission has crossed the accepted lifecycle boundary.
    pub(crate) accepted: Atomic<bool>,
    /// Sender used once by the winner of the terminal state race.
    pub(crate) sender: Mutex<Option<Sender<TaskResult<R, E>>>>,
    /// Optional hook notified when an accepted task starts and finishes.
    pub(crate) hook: Option<Arc<dyn crate::hook::TaskHook>>,
}

impl<R, E> TaskState<R, E> {
    /// Creates shared completion state for a task result sender.
    ///
    /// # Parameters
    ///
    /// * `sender` - One-shot sender used to publish the terminal task result.
    ///
    /// # Returns
    ///
    /// Shared completion state initialized as pending.
    #[inline]
    pub(crate) fn new(
        task_id: TaskId,
        sender: Sender<TaskResult<R, E>>,
        hook: Option<Arc<dyn crate::hook::TaskHook>>,
    ) -> Self {
        Self {
            task_id,
            status: AtomicTaskStatus::new(TaskStatus::Pending),
            accepted: Atomic::new(false),
            sender: Mutex::new(Some(sender)),
            hook,
        }
    }

    /// Marks this task accepted and emits the accepted hook once.
    ///
    /// # Returns
    ///
    /// `true` if this call crossed the accepted boundary, or `false` if another
    /// caller had already marked the task accepted.
    #[inline]
    pub(crate) fn accept(&self) -> bool {
        if self.accepted.swap(true) {
            return false;
        }
        if let Some(hook) = &self.hook {
            crate::hook::notify_accepted(hook.as_ref(), self.task_id);
        }
        true
    }

    /// Returns whether lifecycle hook reporting has been accepted for this task.
    ///
    /// # Returns
    ///
    /// `true` after the task has crossed the accepted lifecycle boundary.
    #[inline]
    pub(crate) fn is_accepted(&self) -> bool {
        self.accepted.load()
    }

    /// Returns the currently observed task status.
    ///
    /// # Returns
    ///
    /// The task status represented by the internal atomic state.
    #[inline]
    pub(crate) fn status(&self) -> TaskStatus {
        self.status.load()
    }

    /// Attempts to move the task from pending to running.
    ///
    /// # Returns
    ///
    /// `true` if this call started the task, or `false` if the task was already
    /// running or terminal.
    #[inline]
    pub(crate) fn try_start(&self, notify_hook: bool) -> bool {
        let started = self.status.try_start();
        if started
            && notify_hook
            && let Some(hook) = &self.hook
        {
            notify_started(hook.as_ref(), self.task_id);
        }
        started
    }

    /// Attempts to cancel this task while it is still pending.
    ///
    /// # Returns
    ///
    /// `true` if this call published a cancellation result.
    #[inline]
    pub(crate) fn try_cancel_pending(&self) -> bool {
        let notify_hook = self.is_accepted();
        let result = Err(TaskExecutionError::Cancelled);
        let status = TaskStatus::from_result(&result);
        if !self.status.try_cancel_pending() {
            return false;
        }
        self.publish_terminal_result(result, notify_hook, status);
        true
    }

    /// Publishes a dropped-result error if no terminal result exists.
    ///
    /// # Returns
    ///
    /// `true` if this call published a dropped-result error.
    #[inline]
    pub(crate) fn try_drop_unfinished(&self, notify_hook: bool) -> bool {
        let result = Err(TaskExecutionError::Dropped);
        let status = TaskStatus::from_result(&result);
        if !self.status.try_drop_unfinished() {
            return false;
        }
        self.publish_terminal_result(result, notify_hook, status);
        true
    }

    /// Attempts to complete a running task with its final result.
    ///
    /// # Parameters
    ///
    /// * `result` - Final task result to publish.
    /// * `notify_hook` - Whether to emit the finished hook after publication.
    ///
    /// # Returns
    ///
    /// `true` if this call published the terminal result, or `false` if the
    /// task was not running or another terminal path already won.
    pub(crate) fn try_complete(&self, result: TaskResult<R, E>, notify_hook: bool) -> bool {
        let status = TaskStatus::from_result(&result);
        if !self.status.try_complete(status) {
            return false;
        }
        self.publish_terminal_result(result, notify_hook, status);
        true
    }

    /// Sends the terminal result and emits the finished hook after a won transition.
    ///
    /// # Parameters
    ///
    /// * `result` - Terminal result to send to the task handle.
    /// * `notify_hook` - Whether to emit the finished hook.
    /// * `status` - Terminal status installed before this call.
    fn publish_terminal_result(&self, result: TaskResult<R, E>, notify_hook: bool, status: TaskStatus) {
        let sender = self.sender.lock().take();
        if let Some(sender) = sender {
            let _ignored = sender.send(result);
        }
        if notify_hook && let Some(hook) = &self.hook {
            notify_finished(hook.as_ref(), self.task_id, status);
        }
    }
}
