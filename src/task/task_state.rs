/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::sync::{
    Arc,
    atomic::{
        AtomicBool,
        Ordering,
    },
};

use oneshot::Sender;
use parking_lot::Mutex;

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
    pub(crate) accepted: AtomicBool,
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
            accepted: AtomicBool::new(false),
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
        if self.accepted.swap(true, Ordering::AcqRel) {
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
        self.accepted.load(Ordering::Acquire)
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
    pub(crate) fn start(&self, notify_hook: bool) -> bool {
        let started = self.status.start();
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
    pub(crate) fn cancel_pending(&self) -> bool {
        self.finish(
            Err(TaskExecutionError::Cancelled),
            self.is_accepted(),
            |status| status == TaskStatus::Pending,
        )
    }

    /// Publishes a dropped-result error if no terminal result exists.
    ///
    /// # Returns
    ///
    /// `true` if this call published a dropped-result error.
    #[inline]
    pub(crate) fn drop_unfinished(&self, notify_hook: bool) -> bool {
        self.finish(Err(TaskExecutionError::Dropped), notify_hook, |_| true)
    }

    /// Attempts to publish a terminal result when the current status allows it.
    ///
    /// # Parameters
    ///
    /// * `result` - Final task result to publish.
    /// * `can_finish` - Predicate deciding whether the current status may
    ///   transition to a terminal state.
    ///
    /// # Returns
    ///
    /// `true` if this call published the terminal result, or `false` if another
    /// path already won or `can_finish` rejected the observed status.
    pub(crate) fn finish<F>(
        &self,
        result: TaskResult<R, E>,
        notify_hook: bool,
        mut can_finish: F,
    ) -> bool
    where
        F: FnMut(TaskStatus) -> bool,
    {
        let next = TaskStatus::from_result(&result);
        loop {
            let current = self.status();
            if current.is_done() || !can_finish(current) {
                return false;
            }
            if self.status.compare_set(current, next) {
                let sender = self.sender.lock().take();
                if let Some(sender) = sender {
                    let _ignored = sender.send(result);
                }
                if notify_hook && let Some(hook) = &self.hook {
                    notify_finished(hook.as_ref(), self.task_id, next);
                }
                return true;
            }
        }
    }
}
