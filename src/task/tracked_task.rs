/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::{
    future::IntoFuture,
    sync::Arc,
};

use super::{
    TaskResult,
    cancel_result::CancelResult,
    task_completion::TaskCompletion,
    task_handle::TaskHandle,
    task_handle_future::TaskHandleFuture,
    task_handle_inner::TaskHandleInner,
    task_result_handle::TaskResultHandle,
    task_status::TaskStatus,
    tracked_task_handle::TrackedTaskHandle,
    try_get::TryGet,
};

/// Result handle with active status tracking and pre-start cancellation.
pub struct TrackedTask<R, E> {
    /// Lightweight result handle.
    handle: TaskHandle<R, E>,
    /// Shared completion state used for status and cancellation.
    inner: Arc<TaskHandleInner<R, E>>,
}

impl<R, E> TrackedTask<R, E> {
    /// Creates a tracked task from a result handle and completion state.
    ///
    /// # Parameters
    ///
    /// * `handle` - Result handle used to retrieve the final task result.
    /// * `inner` - Shared completion state used by the cancellation path.
    ///
    /// # Returns
    ///
    /// A tracked task handle.
    #[inline]
    pub(crate) const fn new(handle: TaskHandle<R, E>, inner: Arc<TaskHandleInner<R, E>>) -> Self {
        Self { handle, inner }
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

    /// Returns whether the tracked task has reached a terminal state.
    ///
    /// # Returns
    ///
    /// `true` after the task succeeds, fails, panics, or is cancelled.
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
        self.inner.status()
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
        let completion = TaskCompletion {
            inner: Arc::clone(&self.inner),
        };
        if completion.cancel() {
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
        let Self { handle, inner } = self;
        match handle.try_get() {
            TryGet::Ready(result) => TryGet::Ready(result),
            TryGet::Pending(handle) => TryGet::Pending(Self { handle, inner }),
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
        self.inner.status()
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
