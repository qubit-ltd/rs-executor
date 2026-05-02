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
    sync::Arc,
    task::{
        Context,
        Poll,
    },
};

use super::TaskResult;
use super::task_completion::TaskCompletion;
use super::task_handle_inner::TaskHandleInner;

/// Handle for a task running outside the caller's current stack.
///
/// `TaskHandle` is returned by thread-backed executors and services. Calling
/// [`Self::get`] blocks the current thread until the accepted task completes.
/// Awaiting the handle waits asynchronously for the same final task result.
///
/// # Type Parameters
///
/// * `R` - The task success value.
/// * `E` - The task error value.
///
pub struct TaskHandle<R, E> {
    /// Shared state observed by the handle and updated by completion endpoints.
    pub(crate) inner: Arc<TaskHandleInner<R, E>>,
}

impl<R, E> TaskHandle<R, E> {
    /// Waits for the task to finish and returns its final result.
    ///
    /// This method blocks the current thread until a result is available.
    ///
    /// # Returns
    ///
    /// `Ok(R)` if the task succeeds. If the accepted task returns `Err(E)`,
    /// panics, or is cancelled before producing a value, the corresponding
    /// [`crate::TaskExecutionError`] is returned.
    pub fn get(self) -> TaskResult<R, E> {
        self.inner.state.wait_until(
            |state| state.completed,
            |state| {
                state
                    .result
                    .take()
                    .expect("task handle completed without a result")
            },
        )
    }

    /// Returns whether the task has reported completion.
    ///
    /// # Returns
    ///
    /// `true` after the task runner has produced or abandoned its final result.
    #[inline]
    pub fn is_done(&self) -> bool {
        self.inner.done.load()
    }

    /// Attempts to cancel the task.
    ///
    /// Cancellation can only win before the runner marks the task as started.
    /// It cannot interrupt a task that is already running on an OS thread.
    ///
    /// # Returns
    ///
    /// `true` if the task was cancelled before it started, or `false` if the
    /// task was already running or completed.
    #[inline]
    pub fn cancel(&self) -> bool {
        TaskCompletion {
            inner: Arc::clone(&self.inner),
        }
        .cancel()
    }
}

impl<R, E> Future for TaskHandle<R, E> {
    type Output = TaskResult<R, E>;

    /// Polls this handle for the accepted task's final result.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result = self.inner.state.write(|state| {
            if state.completed {
                Some(
                    state
                        .result
                        .take()
                        .expect("task handle completed without a result"),
                )
            } else {
                state.waker = Some(cx.waker().clone());
                None
            }
        });
        if let Some(result) = result {
            Poll::Ready(result)
        } else {
            Poll::Pending
        }
    }
}
