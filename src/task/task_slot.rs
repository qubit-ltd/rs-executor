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
    task_status::TaskStatus,
};

/// Runner-side slot for one accepted task.
///
/// This low-level endpoint is exposed so custom executor services built on top
/// of `qubit-executor` can wire their own scheduling while still returning the
/// standard [`crate::TaskHandle`]. Normal callers should use
/// [`crate::TaskHandle`] and executor/service submission methods instead.
pub struct TaskSlot<R, E> {
    /// Shared state updated by this completion endpoint.
    pub(crate) state: Arc<TaskState<R, E>>,
}

impl<R, E> TaskSlot<R, E> {
    /// Marks the task as started if it was not cancelled first.
    ///
    /// # Returns
    ///
    /// `true` if the runner should execute the task, or `false` if the task was
    /// already completed through cancellation.
    pub(crate) fn start(&self) -> bool {
        self.state.start()
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
        self.finish(result, |_| true);
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

    /// Cancels the task if it has not started yet.
    ///
    /// # Returns
    ///
    /// `true` if this call published a cancellation result, or `false` if the
    /// task was already started or completed.
    #[inline]
    pub fn cancel(&self) -> bool {
        self.state.cancel_pending()
    }

    /// Publishes a terminal result when the supplied predicate allows it.
    ///
    /// # Parameters
    ///
    /// * `result` - Terminal result to store.
    /// * `can_finish` - Predicate evaluated against the observed task status to
    ///   decide whether this path may publish the result.
    ///
    /// # Returns
    ///
    /// `true` if the result was published and waiters were notified, or
    /// `false` if another completion path already won or `can_finish`
    /// rejected the transition.
    fn finish<F>(&self, result: TaskResult<R, E>, can_finish: F) -> bool
    where
        F: FnMut(TaskStatus) -> bool,
    {
        self.state.finish(result, can_finish)
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
        let _ignored = self.state.drop_unfinished();
    }
}
