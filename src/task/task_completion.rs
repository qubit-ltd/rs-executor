/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
use std::sync::Arc;

use super::TaskExecutionError;
use super::TaskResult;
use super::task_handle_inner::TaskHandleInner;
use super::task_handle_state::TaskHandleState;

/// Completion endpoint owned by a task runner.
///
/// This low-level endpoint is exposed so custom executor services built on top
/// of `qubit-executor` can wire their own scheduling and cancellation hooks
/// while still returning the standard [`crate::TaskHandle`]. Normal callers
/// should use [`crate::TaskHandle`] and executor/service submission methods
/// instead.
pub struct TaskCompletion<R, E> {
    /// Shared state updated by this completion endpoint.
    pub(crate) inner: Arc<TaskHandleInner<R, E>>,
}

impl<R, E> Clone for TaskCompletion<R, E> {
    /// Clones the completion endpoint for mutually exclusive finish paths.
    ///
    /// # Returns
    ///
    /// A completion endpoint sharing the same task state.
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<R, E> TaskCompletion<R, E> {
    /// Marks the task as started if it was not cancelled first.
    ///
    /// # Returns
    ///
    /// `true` if the runner should execute the task, or `false` if the task was
    /// already completed through cancellation.
    pub fn start(&self) -> bool {
        self.inner.state.write(|state| {
            if state.completed {
                false
            } else {
                state.started = true;
                true
            }
        })
    }

    /// Completes the task with its final result.
    ///
    /// If another path has already completed the task, this result is ignored.
    ///
    /// # Parameters
    ///
    /// * `result` - Final task result to publish if the task is not already
    ///   completed.
    pub fn complete(&self, result: TaskResult<R, E>) {
        self.finish(result, |_| true);
    }

    /// Starts the task and completes it with a lazily produced result.
    ///
    /// The supplied closure is executed only if this completion endpoint wins
    /// the start race. If the handle was cancelled first, the closure is not
    /// called and the existing cancellation result is preserved.
    ///
    /// # Parameters
    ///
    /// * `task` - Closure that runs the accepted task and returns its final
    ///   result.
    ///
    /// # Returns
    ///
    /// `true` if the closure was executed and its result was published, or
    /// `false` if the task had already been completed by cancellation.
    pub fn start_and_complete<F>(&self, task: F) -> bool
    where
        F: FnOnce() -> TaskResult<R, E>,
    {
        if !self.start() {
            return false;
        }
        self.complete(task());
        true
    }

    /// Cancels the task if it has not started yet.
    ///
    /// # Returns
    ///
    /// `true` if this call published a cancellation result, or `false` if the
    /// task was already started or completed.
    pub fn cancel(&self) -> bool {
        self.finish(Err(TaskExecutionError::Cancelled), |state| !state.started)
    }

    /// Publishes a terminal result when the supplied predicate allows it.
    ///
    /// # Parameters
    ///
    /// * `result` - Terminal result to store.
    /// * `can_finish` - Predicate evaluated under the monitor lock to decide
    ///   whether this path may publish the result.
    ///
    /// # Returns
    ///
    /// `true` if the result was published and waiters were notified, or
    /// `false` if another completion path already won or `can_finish`
    /// rejected the transition.
    fn finish<F>(&self, result: TaskResult<R, E>, can_finish: F) -> bool
    where
        F: FnOnce(&TaskHandleState<R, E>) -> bool,
    {
        let (published, waker) = self.inner.state.write(|state| {
            if state.completed || !can_finish(state) {
                return (false, None);
            }
            state.result = Some(result);
            state.completed = true;
            self.inner.done.store(true);
            (true, state.waker.take())
        });
        if !published {
            return false;
        }
        self.inner.notify_completion();
        if let Some(waker) = waker {
            waker.wake();
        }
        true
    }
}
