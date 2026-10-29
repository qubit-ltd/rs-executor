// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use oneshot::AsyncReceiver;

use super::TaskExecutionError;
use super::TaskResult;

/// Future returned when awaiting a task handle by value.
///
/// Polling this future yields the task's final result. If the result sender is
/// dropped before publishing a result, the future resolves to
/// [`TaskExecutionError::Dropped`].
///
/// # Type Parameters
///
/// * `R` - The value returned when the task succeeds.
/// * `E` - The error returned when the task fails.
///
/// A `TaskHandleFuture` owns the wait for a submitted task's result. Dropping
/// this future abandons the wait; it does not actively cancel the submitted
/// task.
///
/// # Examples
///
/// The async function below shows a runtime-independent way to await a task
/// result. The example demonstrates the async usage at compile time; it does
/// not execute the body of `example` as part of the doctest.
///
/// ```rust
/// use std::future::IntoFuture;
/// use std::io;
///
/// use qubit_executor::ExecutorService;
/// use qubit_executor::ThreadPerTaskExecutorService;
/// use qubit_executor::task::TaskHandleFuture;
///
/// async fn example() -> Result<(), Box<dyn std::error::Error>> {
///     let service = ThreadPerTaskExecutorService::new();
///     let handle = service.submit_callable(|| Ok::<usize, io::Error>(42))?;
///     let future: TaskHandleFuture<usize, io::Error> = handle.into_future();
///     let result = future.await;
///     service.shutdown();
///     service.wait_termination();
///     assert_eq!(result?, 42);
///     Ok(())
/// }
/// ```
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
    ///
    /// # Parameters
    ///
    /// * `self` - Pinned mutable access to this future.
    /// * `cx` - Context used to arrange another poll when the result is not
    ///   ready.
    ///
    /// # Returns
    ///
    /// `Poll::Pending` while the task has not produced a result, or
    /// `Poll::Ready` with the result. If the sender was dropped, the ready
    /// value contains [`TaskExecutionError::Dropped`].
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        Pin::new(&mut this.receiver)
            .poll(cx)
            .map(|result| result.unwrap_or(Err(TaskExecutionError::Dropped)))
    }
}
