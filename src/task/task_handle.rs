/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use oneshot::{
    Receiver,
    TryRecvError,
};
use std::future::IntoFuture;

use super::{
    TaskExecutionError,
    TaskResult,
    task_handle_future::TaskHandleFuture,
    task_result_handle::TaskResultHandle,
    try_get::TryGet,
};

/// Lightweight result handle for a submitted callable task.
///
/// `TaskHandle` owns the receiving endpoint for exactly one task result. It can
/// block through [`Self::get`], poll non-blockingly through [`Self::try_get`],
/// or be awaited by value.
pub struct TaskHandle<R, E> {
    /// One-shot receiver for the final task result.
    receiver: Receiver<TaskResult<R, E>>,
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
    pub(crate) const fn new(receiver: Receiver<TaskResult<R, E>>) -> Self {
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
            Err(TryRecvError::Empty) => TryGet::Pending(self),
            Err(TryRecvError::Disconnected) => TryGet::Ready(Err(TaskExecutionError::Cancelled)),
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

impl<R, E> IntoFuture for TaskHandle<R, E> {
    type Output = TaskResult<R, E>;
    type IntoFuture = TaskHandleFuture<R, E>;

    /// Converts this handle into a future resolving to the task result.
    #[inline]
    fn into_future(self) -> Self::IntoFuture {
        TaskHandleFuture::new(IntoFuture::into_future(self.receiver))
    }
}
