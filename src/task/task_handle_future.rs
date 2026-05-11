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
    future::Future,
    pin::Pin,
    task::{
        Context,
        Poll,
    },
};

use oneshot::AsyncReceiver;

use super::{
    TaskExecutionError,
    TaskResult,
};

/// Future returned when awaiting a task handle by value.
pub struct TaskHandleFuture<R, E> {
    /// Async receiver created from the task result receiver.
    receiver: AsyncReceiver<TaskResult<R, E>>,
}

impl<R, E> TaskHandleFuture<R, E> {
    /// Creates a task-handle future from an async result receiver.
    ///
    /// # Parameters
    ///
    /// * `receiver` - Async receiver that resolves to the final task result.
    ///
    /// # Returns
    ///
    /// A future resolving to the final task result.
    #[inline]
    pub(crate) const fn new(receiver: AsyncReceiver<TaskResult<R, E>>) -> Self {
        Self { receiver }
    }
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
