// =============================================================================
// qubit-style: allow source-test-pair
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;

use qubit_function::Callable;

use super::{TaskResult, task_runner::TaskRunner, task_state::TaskState};

/// Runner-side slot for a task that has crossed into the running state.
///
/// This endpoint is returned by
/// [`TaskSlot::try_start`](crate::task::spi::TaskSlot::try_start) after the
/// pending to running transition succeeds. It lets schedulers claim a task
/// under their own queue locks before removing the task from their pending
/// structures. Dropping a running slot before completion reports
/// [`crate::TaskExecutionError::Dropped`].
pub struct RunningTaskSlot<R, E> {
    /// Shared state updated by this running endpoint.
    state: Option<Arc<TaskState<R, E>>>,
}

impl<R, E> RunningTaskSlot<R, E> {
    /// Creates a running task slot from already-started task state.
    ///
    /// # Parameters
    ///
    /// * `state` - Shared task state that has already moved to running.
    ///
    /// # Returns
    ///
    /// A running endpoint that must complete or drop the task.
    #[inline]
    pub(crate) fn new(state: Arc<TaskState<R, E>>) -> Self {
        Self { state: Some(state) }
    }

    /// Returns the shared state owned by this running slot.
    ///
    /// # Returns
    ///
    /// A reference to the task state. The state is present until this running
    /// endpoint publishes a terminal completion result.
    #[inline]
    fn state(&self) -> &TaskState<R, E> {
        self.state
            .as_deref()
            .expect("running task slot state should be present")
    }

    /// Completes the running task with its final result.
    ///
    /// Cancellation is a pre-start decision and should be reported through
    /// [`TaskSlot::cancel_unstarted`](crate::task::spi::TaskSlot::cancel_unstarted).
    /// Passing a cancellation or dropped result does not complete the running
    /// task; the slot is then dropped and callers observe
    /// [`crate::TaskExecutionError::Dropped`].
    ///
    /// # Parameters
    ///
    /// * `result` - Final task result to publish for this running task.
    ///
    /// # Returns
    ///
    /// `true` if the result was published, or `false` if the state was no
    /// longer running or the result does not represent normal task completion.
    #[inline]
    pub fn complete(mut self, result: TaskResult<R, E>) -> bool {
        let completed = self
            .state()
            .try_complete(result, self.state().is_accepted());
        if completed {
            self.state.take();
        }
        completed
    }

    /// Runs a callable and publishes its final result through this running
    /// slot.
    ///
    /// The callable is always executed because the pending-to-running
    /// transition has already succeeded. Task failures and panics are converted
    /// through [`TaskRunner`].
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to execute for the already-started task.
    ///
    /// # Returns
    ///
    /// `true` if the callable result was published, or `false` if completion
    /// was no longer possible.
    #[inline]
    pub fn run<C>(self, task: C) -> bool
    where
        C: Callable<R, E>,
    {
        self.complete(TaskRunner::new(task).call())
    }
}

impl<R, E> Drop for RunningTaskSlot<R, E> {
    /// Publishes a dropped-result error when the running endpoint is abandoned.
    #[inline]
    fn drop(&mut self) {
        if let Some(state) = &self.state {
            let _ignored = state.try_drop_unfinished(state.is_accepted());
        }
    }
}
