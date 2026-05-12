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

use qubit_function::Callable;

use super::{
    TaskResult,
    task_runner::TaskRunner,
    task_state::TaskState,
};

/// Runner-side slot for one task submission.
///
/// This low-level endpoint is exposed so custom executor services built on top
/// of `qubit-executor` can wire their own scheduling while still returning the
/// standard [`crate::TaskHandle`]. Executor implementations should call
/// [`Self::accept`] only after submission succeeds; this arms lifecycle hook
/// reporting for later start and finish events. Normal callers should use
/// [`crate::TaskHandle`] and executor/service submission methods instead.
///
/// Dropping an accepted slot reports [`crate::TaskExecutionError::Dropped`]
/// because it means the runner endpoint was abandoned without making an
/// explicit terminal decision. Executor services that intentionally discard
/// accepted work before it starts, such as during
/// [`crate::ExecutorService::stop`], should call [`Self::cancel_unstarted`] so
/// callers observe [`crate::TaskExecutionError::Cancelled`] instead.
pub struct TaskSlot<R, E> {
    /// Shared state updated by this completion endpoint.
    pub(crate) state: Arc<TaskState<R, E>>,
}

impl<R, E> TaskSlot<R, E> {
    /// Marks this runner endpoint as accepted and arms lifecycle hook reporting.
    ///
    /// Calling this method emits `on_accepted` before any later `on_started` or
    /// `on_finished` event for the same task. Executor implementations must call
    /// it only after submission has succeeded. Dropping a slot before acceptance
    /// still releases result waiters with `Dropped`, but does not emit lifecycle
    /// hook events for a task that was rejected before acceptance.
    #[inline]
    pub fn accept(&self) {
        let _accepted_now = self.state.accept();
    }

    /// Cancels this accepted runner endpoint before it starts running.
    ///
    /// This method is the runner-side service-provider API for an executor or
    /// executor service that intentionally removes queued, scheduled, or other
    /// unstarted accepted work. It publishes
    /// [`crate::TaskExecutionError::Cancelled`] when this slot wins the
    /// pending-task terminal-state race. The slot is consumed to make the
    /// explicit cancellation decision the final runner-side action.
    ///
    /// If the slot has already been accepted, successful cancellation emits the
    /// finished lifecycle hook with [`crate::TaskStatus::Cancelled`]. If it has
    /// not been accepted, cancellation still releases result waiters but does
    /// not emit lifecycle hook events.
    ///
    /// # Returns
    ///
    /// `true` if this call moved the task from pending to cancelled, or `false`
    /// if another path had already started or completed the task.
    #[inline]
    pub fn cancel_unstarted(self) -> bool {
        self.state.try_cancel_pending()
    }

    /// Marks the task as started if it was not cancelled first.
    ///
    /// # Returns
    ///
    /// `true` if the runner should execute the task, or `false` if the task was
    /// already completed through cancellation.
    pub(crate) fn start(&self) -> bool {
        self.state.try_start(self.state.is_accepted())
    }

    /// Completes the task with its final result.
    ///
    /// If another path has already completed the task, this result is ignored.
    ///
    /// # Parameters
    ///
    /// * `result` - Final task result to publish if the task is not already
    ///   completed.
    #[inline]
    pub(crate) fn complete(&self, result: TaskResult<R, E>) {
        let _completed = self.state.try_complete(result, self.state.is_accepted());
    }

    /// Starts the task and completes it with a lazily produced result.
    ///
    /// The supplied closure is executed only if this completion endpoint wins
    /// the start race. If the handle was cancelled first, the closure is not
    /// called and the existing cancellation result is preserved.
    ///
    /// # Parameters
    ///
    /// * `task` - Closure that runs the accepted task and returns its final
    ///   result.
    ///
    /// # Returns
    ///
    /// `true` if the closure was executed and its result was published, or
    /// `false` if the task had already been completed by cancellation.
    #[inline]
    pub(crate) fn start_and_complete<F>(&self, task: F) -> bool
    where
        F: FnOnce() -> TaskResult<R, E>,
    {
        if !self.start() {
            return false;
        }
        self.complete(task());
        true
    }

    /// Starts this slot and runs a callable to completion.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to run if the task has not been cancelled.
    ///
    /// # Returns
    ///
    /// `true` if the callable ran and published a result, or `false` if the
    /// task had already been cancelled.
    #[inline]
    pub fn run<C>(self, task: C) -> bool
    where
        C: Callable<R, E>,
    {
        self.start_and_complete(|| TaskRunner::new(task).call())
    }
}

impl<R, E> Drop for TaskSlot<R, E> {
    /// Publishes a dropped-result error when the runner endpoint is abandoned.
    #[inline]
    fn drop(&mut self) {
        let _ignored = self.state.try_drop_unfinished(self.state.is_accepted());
    }
}
