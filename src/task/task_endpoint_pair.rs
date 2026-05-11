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

use oneshot::Receiver;
use oneshot::channel;

use super::task_completer::TaskCompleter;
use super::task_execution_error::TaskResult;
use super::task_handle::TaskHandle;
use super::task_handle_inner::TaskHandleInner;
use super::tracked_task::TrackedTask;

/// One-shot pair of endpoints for an accepted task.
///
/// A pair owns the shared task completion endpoint and the result receiver
/// until it is split into caller-facing and runner-facing endpoints.
pub struct TaskEndpointPair<R, E> {
    /// Receiver consumed by the caller-facing handle.
    receiver: Receiver<TaskResult<R, E>>,
    /// Shared completion state consumed by the runner-facing endpoint.
    inner: Arc<TaskHandleInner<R, E>>,
}

impl<R, E> TaskEndpointPair<R, E> {
    /// Creates a new unsplit task completion pair.
    ///
    /// # Returns
    ///
    /// A pair that can be split once into its handle and completion endpoints.
    #[inline]
    pub fn new() -> Self {
        let (sender, receiver) = channel();
        Self {
            receiver,
            inner: Arc::new(TaskHandleInner::new(sender)),
        }
    }

    /// Splits this pair into a result handle and completion endpoint.
    ///
    /// # Returns
    ///
    /// A [`TaskHandle`] for the caller and a [`TaskCompleter`] for the runner.
    #[inline]
    pub fn into_parts(self) -> (TaskHandle<R, E>, TaskCompleter<R, E>) {
        let handle = TaskHandle::new(self.receiver);
        let completion = TaskCompleter { inner: self.inner };
        (handle, completion)
    }

    /// Splits this pair into a tracked result handle and completion endpoint.
    ///
    /// # Returns
    ///
    /// A [`TrackedTask`] for the caller and a [`TaskCompleter`] for the runner.
    #[inline]
    pub fn into_tracked_parts(self) -> (TrackedTask<R, E>, TaskCompleter<R, E>) {
        let handle = TaskHandle::new(self.receiver);
        let tracked = TrackedTask::new(handle, Arc::clone(&self.inner));
        let completion = TaskCompleter { inner: self.inner };
        (tracked, completion)
    }
}

impl<R, E> Default for TaskEndpointPair<R, E> {
    /// Creates a new unsplit task completion pair.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
