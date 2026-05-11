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

use crate::hook::{
    NoopTaskHook,
    TaskHook,
    next_task_id,
};

use super::task_execution_error::TaskResult;
use super::task_handle::TaskHandle;
use super::task_slot::TaskSlot;
use super::task_state::TaskState;
use super::tracked_task::TrackedTask;

/// One-shot pair of endpoints for an accepted task.
///
/// A pair owns the shared task completion endpoint and the result receiver
/// until it is split into caller-facing and runner-facing endpoints.
pub struct TaskEndpointPair<R, E> {
    /// Receiver consumed by the caller-facing handle.
    receiver: Receiver<TaskResult<R, E>>,
    /// Shared completion state consumed by the runner-facing endpoint.
    state: Arc<TaskState<R, E>>,
}

impl<R, E> TaskEndpointPair<R, E> {
    /// Creates a new unsplit task completion pair.
    ///
    /// # Returns
    ///
    /// A pair that can be split once into its handle and completion endpoints.
    #[inline]
    pub fn new() -> Self {
        Self::with_hook(Arc::new(NoopTaskHook))
    }

    /// Creates a new unsplit task completion pair with a lifecycle hook.
    ///
    /// # Parameters
    ///
    /// * `hook` - Hook notified about this task's lifecycle.
    ///
    /// # Returns
    ///
    /// A pair that can be split once into its handle and runner slot.
    #[inline]
    pub fn with_hook(hook: Arc<dyn TaskHook>) -> Self {
        let (sender, receiver) = channel();
        Self {
            receiver,
            state: Arc::new(TaskState::new(next_task_id(), sender, hook)),
        }
    }

    /// Splits this pair into a result handle and completion endpoint.
    ///
    /// # Returns
    ///
    /// A [`TaskHandle`] for the caller and a [`TaskSlot`] for the runner.
    #[inline]
    pub fn into_parts(self) -> (TaskHandle<R, E>, TaskSlot<R, E>) {
        let handle = TaskHandle::new(Arc::clone(&self.state), self.receiver);
        let slot = TaskSlot { state: self.state };
        (handle, slot)
    }

    /// Splits this pair into a tracked result handle and completion endpoint.
    ///
    /// # Returns
    ///
    /// A [`TrackedTask`] for the caller and a [`TaskSlot`] for the runner.
    #[inline]
    pub fn into_tracked_parts(self) -> (TrackedTask<R, E>, TaskSlot<R, E>) {
        let handle = TaskHandle::new(Arc::clone(&self.state), self.receiver);
        let tracked = TrackedTask::new(handle);
        let slot = TaskSlot { state: self.state };
        (tracked, slot)
    }
}

impl<R, E> Default for TaskEndpointPair<R, E> {
    /// Creates a new unsplit task completion pair.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
