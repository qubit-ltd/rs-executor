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
    future::{Future, IntoFuture},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use super::{
    TaskExecutionError, TaskResult,
    task_completion::TaskCompletion,
    task_handle_inner::TaskHandleInner,
    task_handle_state::{CancelResult, TaskStatus},
};

/// Result of a non-blocking attempt to retrieve a task result.
pub enum TryGet<H, R, E> {
    /// The task result is ready.
    Ready(TaskResult<R, E>),
    /// The task has not completed and the handle is returned to the caller.
    Pending(H),
}

/// Common interface for handles that expose a submitted task's final result.
pub trait TaskResultHandle<R, E>: Send {
    /// Returns whether the task result is ready or otherwise terminal.
    ///
    /// # Returns
    ///
    /// `true` after the task result can be retrieved or the completion channel
    /// has been closed.
    fn is_done(&self) -> bool;

    /// Blocks until the task produces its final result.
    ///
    /// # Returns
    ///
    /// The final task result. If the completion endpoint is dropped without
    /// publishing a result, cancellation is reported.
    fn get(self) -> TaskResult<R, E>
    where
        Self: Sized;

    /// Attempts to retrieve the final result without blocking.
    ///
    /// # Returns
    ///
    /// [`TryGet::Ready`] when a result is available, otherwise
    /// [`TryGet::Pending`] containing the original handle.
    fn try_get(self) -> TryGet<Self, R, E>
    where
        Self: Sized;
}

/// Extension interface for handles that expose active task tracking.
pub trait TrackedTaskHandle<R, E>: TaskResultHandle<R, E> {
    /// Returns the currently observed task status.
    ///
    /// # Returns
    ///
    /// The current pending, running, or terminal task status.
    fn status(&self) -> TaskStatus;

    /// Attempts to cancel the task before it starts.
    ///
    /// # Returns
    ///
    /// A precise cancellation result describing whether cancellation won.
    fn cancel(&self) -> CancelResult;
}

/// Lightweight result handle for a submitted callable task.
///
/// `TaskHandle` owns the receiving endpoint for exactly one task result. It can
/// block through [`Self::get`], poll non-blockingly through [`Self::try_get`],
/// or be awaited by value.
pub struct TaskHandle<R, E> {
    /// One-shot receiver for the final task result.
    receiver: oneshot::Receiver<TaskResult<R, E>>,
}

impl<R, E> TaskHandle<R, E> {
    /// Creates a task handle from a one-shot result receiver.
    ///
    /// # Parameters
    ///
    /// * `receiver` - Receiver that yields the final task result.
    ///
    /// # Returns
    ///
    /// A task result handle.
    #[inline]
    pub(crate) const fn new(receiver: oneshot::Receiver<TaskResult<R, E>>) -> Self {
        Self { receiver }
    }

    /// Waits for the task to finish and returns its final result.
    ///
    /// This method blocks the current thread until a result is available.
    ///
    /// # Returns
    ///
    /// `Ok(R)` if the task succeeds. If the accepted task returns `Err(E)`,
    /// panics, or is cancelled before producing a value, the corresponding
    /// [`crate::TaskExecutionError`] is returned.
    #[inline]
    pub fn get(self) -> TaskResult<R, E> {
        self.receiver
            .recv()
            .unwrap_or(Err(TaskExecutionError::Cancelled))
    }

    /// Attempts to retrieve the final result without blocking.
    ///
    /// # Returns
    ///
    /// [`TryGet::Ready`] with the final result when available, otherwise
    /// [`TryGet::Pending`] containing this handle.
    #[inline]
    pub fn try_get(self) -> TryGet<Self, R, E> {
        match self.receiver.try_recv() {
            Ok(result) => TryGet::Ready(result),
            Err(oneshot::TryRecvError::Empty) => TryGet::Pending(self),
            Err(oneshot::TryRecvError::Disconnected) => {
                TryGet::Ready(Err(TaskExecutionError::Cancelled))
            }
        }
    }

    /// Returns whether the task has reported completion.
    ///
    /// # Returns
    ///
    /// `true` after the task runner has produced or abandoned its final result.
    #[inline]
    pub fn is_done(&self) -> bool {
        self.receiver.has_message() || self.receiver.is_closed()
    }
}

impl<R, E> TaskResultHandle<R, E> for TaskHandle<R, E>
where
    R: Send,
    E: Send,
{
    /// Returns whether the result channel has a message or is closed.
    #[inline]
    fn is_done(&self) -> bool {
        Self::is_done(self)
    }

    /// Blocks until the result channel yields a task result.
    #[inline]
    fn get(self) -> TaskResult<R, E> {
        Self::get(self)
    }

    /// Attempts to read the result channel without blocking.
    #[inline]
    fn try_get(self) -> TryGet<Self, R, E> {
        Self::try_get(self)
    }
}

/// Future returned when awaiting a [`TaskHandle`] by value.
pub struct TaskHandleFuture<R, E> {
    /// Async receiver created from the task result receiver.
    receiver: oneshot::AsyncReceiver<TaskResult<R, E>>,
}

impl<R, E> Future for TaskHandleFuture<R, E> {
    type Output = TaskResult<R, E>;

    /// Polls the result receiver for the final task result.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        Pin::new(&mut this.receiver)
            .poll(cx)
            .map(|result| result.unwrap_or(Err(TaskExecutionError::Cancelled)))
    }
}

impl<R, E> IntoFuture for TaskHandle<R, E> {
    type Output = TaskResult<R, E>;
    type IntoFuture = TaskHandleFuture<R, E>;

    /// Converts this handle into a future resolving to the task result.
    #[inline]
    fn into_future(self) -> Self::IntoFuture {
        TaskHandleFuture {
            receiver: IntoFuture::into_future(self.receiver),
        }
    }
}

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

impl<R, E> IntoFuture for TrackedTask<R, E> {
    type Output = TaskResult<R, E>;
    type IntoFuture = TaskHandleFuture<R, E>;

    /// Converts this tracked handle into a future resolving to the task result.
    #[inline]
    fn into_future(self) -> Self::IntoFuture {
        self.handle.into_future()
    }
}
