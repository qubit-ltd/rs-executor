// =============================================================================
// qubit-style: allow source-test-pair
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::future::IntoFuture;

use super::{
    TaskResult,
    cancel_result::CancelResult,
    task_handle::TaskHandle,
    task_handle_future::TaskHandleFuture,
    task_result_handle::TaskResultHandle,
    task_status::TaskStatus,
    tracked_task_handle::TrackedTaskHandle,
    try_get::TryGet,
};
use crate::hook::TaskId;

/// Result handle with active status tracking and pre-start cancellation.
pub struct TrackedTask<R, E> {
    /// Lightweight result handle.
    handle: TaskHandle<R, E>,
}

impl<R, E> TrackedTask<R, E> {
    /// Creates a tracked task from a result handle and completion state.
    ///
    /// # Parameters
    ///
    /// * `handle` - Result handle used to retrieve the final task result.
    /// # Returns
    ///
    /// A tracked task handle.
    #[inline]
    pub(crate) const fn new(handle: TaskHandle<R, E>) -> Self {
        Self { handle }
    }

    /// Waits for the task to finish and returns its final result.
    ///
    /// # Returns
    ///
    /// The final task result.
    #[inline]
    pub fn get(self) -> TaskResult<R, E>
    where
        R: Send,
        E: Send,
    {
        <Self as TaskResultHandle<R, E>>::get(self)
    }

    /// Attempts to retrieve the final result without blocking.
    ///
    /// # Returns
    ///
    /// A ready result or the pending tracked handle.
    #[inline]
    pub fn try_get(self) -> TryGet<Self, R, E>
    where
        R: Send,
        E: Send,
    {
        <Self as TaskResultHandle<R, E>>::try_get(self)
    }

    /// Returns whether the tracked task has installed a terminal state.
    ///
    /// # Returns
    ///
    /// `true` after the task succeeds, fails, panics, is cancelled, or loses
    /// its completion endpoint. The final result send may still be racing with
    /// this status observation.
    #[inline]
    pub fn is_done(&self) -> bool
    where
        R: Send,
        E: Send,
    {
        <Self as TaskResultHandle<R, E>>::is_done(self)
    }

    /// Returns the currently observed task status.
    ///
    /// # Returns
    ///
    /// The current task status.
    #[inline]
    pub fn status(&self) -> TaskStatus {
        self.handle.state.status()
    }

    /// Returns the identifier assigned to this task.
    ///
    /// # Returns
    ///
    /// The task id stored in the shared task state.
    #[inline]
    pub fn task_id(&self) -> TaskId {
        self.handle.task_id()
    }

    /// Marks the task accepted and emits the accepted hook once.
    #[inline]
    pub(crate) fn accept(&self) {
        self.handle.accept();
    }

    /// Attempts to cancel this task before it starts.
    ///
    /// # Returns
    ///
    /// The observed cancellation outcome.
    #[inline]
    pub fn cancel(&self) -> CancelResult {
        self.cancel_inner()
    }

    /// Performs the shared cancellation state transition.
    ///
    /// # Returns
    ///
    /// The observed cancellation outcome.
    #[inline]
    fn cancel_inner(&self) -> CancelResult {
        if self.handle.state.try_cancel_pending() {
            return CancelResult::Cancelled;
        }
        match self.status() {
            TaskStatus::Pending => CancelResult::Unsupported,
            TaskStatus::Running => CancelResult::AlreadyRunning,
            _ => CancelResult::AlreadyFinished,
        }
    }
}

impl<R, E> TaskResultHandle<R, E> for TrackedTask<R, E>
where
    R: Send,
    E: Send,
{
    /// Returns whether the tracked state is terminal.
    #[inline]
    fn is_done(&self) -> bool {
        self.status().is_done()
    }

    /// Blocks until the underlying result handle yields a result.
    #[inline]
    fn get(self) -> TaskResult<R, E> {
        self.handle.get()
    }

    /// Attempts to retrieve the underlying result without blocking.
    #[inline]
    fn try_get(self) -> TryGet<Self, R, E> {
        let Self { handle } = self;
        match handle.try_get() {
            TryGet::Ready(result) => TryGet::Ready(result),
            TryGet::Pending(handle) => TryGet::Pending(Self { handle }),
        }
    }
}

impl<R, E> TrackedTaskHandle<R, E> for TrackedTask<R, E>
where
    R: Send,
    E: Send,
{
    /// Returns the currently observed task status.
    #[inline]
    fn status(&self) -> TaskStatus {
        self.handle.state.status()
    }

    /// Attempts to publish a cancellation result while the task is pending.
    #[inline]
    fn cancel(&self) -> CancelResult {
        self.cancel_inner()
    }
}

impl<R, E> IntoFuture for TrackedTask<R, E> {
    type Output = TaskResult<R, E>;
    type IntoFuture = TaskHandleFuture<R, E>;

    /// Converts this tracked handle into a future resolving to the task result.
    #[inline]
    fn into_future(self) -> Self::IntoFuture {
        self.handle.into_future()
    }
}
