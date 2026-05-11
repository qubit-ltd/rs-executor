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

use super::task_completion::TaskCompletion;
use super::task_handle::{TaskHandle, TrackedTask};
use super::task_handle_inner::TaskHandleInner;

/// One-shot pair of endpoints for an accepted task.
///
/// A pair owns the shared task completion endpoint and the result receiver
/// until it is split into caller-facing and runner-facing endpoints.
pub struct TaskCompletionPair<R, E> {
    /// Receiver consumed by the caller-facing handle.
    receiver: Option<oneshot::Receiver<super::TaskResult<R, E>>>,
    /// Shared completion state consumed by the runner-facing endpoint.
    inner: Arc<TaskHandleInner<R, E>>,
}

impl<R, E> TaskCompletionPair<R, E> {
    /// Creates a new unsplit task completion pair.
    ///
    /// # Returns
    ///
    /// A pair that can be split once into its handle and completion endpoints.
    #[inline]
    pub fn new() -> Self {
        let (sender, receiver) = oneshot::channel();
        Self {
            receiver: Some(receiver),
            inner: Arc::new(TaskHandleInner::new(sender)),
        }
    }

    /// Splits this pair into a result handle and completion endpoint.
    ///
    /// # Returns
    ///
    /// A [`TaskHandle`] for the caller and a [`TaskCompletion`] for the runner.
    #[inline]
    pub fn into_parts(mut self) -> (TaskHandle<R, E>, TaskCompletion<R, E>) {
        let receiver = self
            .receiver
            .take()
            .expect("task completion pair receiver already consumed");
        let handle = TaskHandle::new(receiver);
        let completion = TaskCompletion { inner: self.inner };
        (handle, completion)
    }

    /// Splits this pair into a tracked result handle and completion endpoint.
    ///
    /// # Returns
    ///
    /// A [`TrackedTask`] for the caller and a [`TaskCompletion`] for the runner.
    #[inline]
    pub fn into_tracked_parts(mut self) -> (TrackedTask<R, E>, TaskCompletion<R, E>) {
        let receiver = self
            .receiver
            .take()
            .expect("task completion pair receiver already consumed");
        let handle = TaskHandle::new(receiver);
        let tracked = TrackedTask::new(handle, Arc::clone(&self.inner));
        let completion = TaskCompletion { inner: self.inner };
        (tracked, completion)
    }
}

impl<R, E> Default for TaskCompletionPair<R, E> {
    /// Creates a new unsplit task completion pair.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
