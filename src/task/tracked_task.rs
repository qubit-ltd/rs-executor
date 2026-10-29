// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::future::IntoFuture;
use std::sync::Arc;

use super::TaskResult;
use super::cancel_result::CancelResult;
use super::internal::PendingTaskWait;
use super::task_handle::TaskHandle;
use super::task_handle_future::TaskHandleFuture;
use super::task_result_handle::TaskResultHandle;
use super::task_status::TaskStatus;
use super::tracked_task_handle::TrackedTaskHandle;
use super::try_get::TryGet;
use crate::hook::TaskId;

/// Result handle with active status tracking and pre-start cancellation.
///
/// # Type Parameters
///
/// * `R` - Value produced when the task succeeds.
/// * `E` - Error value produced when the task returns an error.
///
/// # Examples
///
/// ```
/// use qubit_executor::task::spi::TaskEndpointPair;
///
/// let (tracked, slot) = TaskEndpointPair::<usize, ()>::new().into_tracked_parts();
/// assert!(slot.run(|| Ok(42)));
/// assert_eq!(tracked.get().expect("task should succeed"), 42);
/// ```
pub struct TrackedTask<R, E> {
    /// Lightweight result handle.
    handle: TaskHandle<R, E>,
    /// Optional wakeup for an executor-owned pending delay.
    pending_wait: Option<Arc<PendingTaskWait>>,
}

impl<R, E> TrackedTask<R, E> {
    /// Creates a tracked task from a result handle.
    ///
    /// # Parameters
    ///
    /// * `handle` - Result handle used to retrieve the final task result.
    /// # Returns
    ///
    /// A tracked task handle.
    #[inline]
    pub(crate) const fn new(handle: TaskHandle<R, E>) -> Self {
        Self {
            handle,
            pending_wait: None,
        }
    }

    /// Attaches the executor's pending delay wakeup to this tracked handle.
    #[inline]
    pub(crate) fn with_pending_wait(mut self, pending_wait: Arc<PendingTaskWait>) -> Self {
        self.pending_wait = Some(pending_wait);
        self
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
    ///
    /// This method consumes `self`. If the task is still pending, the returned
    /// value contains the original tracked handle so it can be polled again.
    /// To intentionally discard either outcome, write `let _ =` explicitly.
    ///
    /// ```rust,compile_fail
    /// #![deny(unused_must_use)]
    /// use qubit_executor::task::spi::TaskEndpointPair;
    ///
    /// let (tracked, _slot) = TaskEndpointPair::<(), ()>::new().into_tracked_parts();
    /// tracked.try_get();
    /// ```
    ///
    /// Handle both possible outcomes to retain a pending task for later:
    ///
    /// ```
    /// use qubit_executor::TryGet;
    /// use qubit_executor::task::spi::TaskEndpointPair;
    ///
    /// let (tracked, slot) = TaskEndpointPair::<usize, ()>::new().into_tracked_parts();
    /// let tracked = match tracked.try_get() {
    ///     TryGet::Pending(tracked) => tracked,
    ///     TryGet::Ready(_) => panic!("unstarted task should be pending"),
    /// };
    /// slot.run(|| Ok::<usize, ()>(42));
    /// assert_eq!(tracked.get().expect("published result"), 42);
    /// ```
    #[must_use = "inspect the ready result or retain the pending task handle"]
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
    #[must_use]
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
    #[must_use]
    #[inline]
    pub fn status(&self) -> TaskStatus {
        self.handle.state.status()
    }

    /// Returns the identifier assigned to this task.
    ///
    /// # Returns
    ///
    /// The task id stored in the shared task state.
    #[must_use]
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
            if let Some(pending_wait) = &self.pending_wait {
                pending_wait.cancel();
            }
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
        self.handle.state.is_done()
    }

    /// Blocks until the underlying result handle yields a result.
    #[inline]
    fn get(self) -> TaskResult<R, E> {
        self.handle.get()
    }

    /// Attempts to retrieve the underlying result without blocking.
    #[inline]
    fn try_get(self) -> TryGet<Self, R, E> {
        let Self { handle, pending_wait } = self;
        match handle.try_get() {
            TryGet::Ready(result) => TryGet::Ready(result),
            TryGet::Pending(handle) => TryGet::Pending(Self { handle, pending_wait }),
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
