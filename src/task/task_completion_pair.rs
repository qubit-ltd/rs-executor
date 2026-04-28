/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
use std::sync::Arc;

use qubit_atomic::Atomic;
use qubit_lock::Monitor;

use super::task_completion::TaskCompletion;
use super::task_handle::TaskHandle;
use super::task_handle_inner::TaskHandleInner;
use super::task_handle_state::TaskHandleState;

/// One-shot pair of endpoints for an accepted task.
///
/// A pair owns the shared task state until it is split into a caller-facing
/// [`TaskHandle`] and a runner-facing [`TaskCompletion`].
pub struct TaskCompletionPair<R, E> {
    /// Shared state consumed when the pair is split.
    inner: Arc<TaskHandleInner<R, E>>,
}

impl<R, E> TaskCompletionPair<R, E> {
    /// Creates a new unsplit task completion pair.
    ///
    /// # Returns
    ///
    /// A pair that can be split once into its handle and completion endpoints.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(TaskHandleInner {
                state: Monitor::new(TaskHandleState {
                    result: None,
                    started: false,
                    completed: false,
                    waker: None,
                }),
                done: Atomic::new(false),
            }),
        }
    }

    /// Splits this pair into caller and runner endpoints.
    ///
    /// # Returns
    ///
    /// A [`TaskHandle`] for the caller and a [`TaskCompletion`] for the runner.
    pub fn into_parts(self) -> (TaskHandle<R, E>, TaskCompletion<R, E>) {
        let handle = TaskHandle {
            inner: Arc::clone(&self.inner),
        };
        let completion = TaskCompletion { inner: self.inner };
        (handle, completion)
    }
}

impl<R, E> Default for TaskCompletionPair<R, E> {
    /// Creates a new unsplit task completion pair.
    fn default() -> Self {
        Self::new()
    }
}
