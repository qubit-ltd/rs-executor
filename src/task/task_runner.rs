/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
use std::panic::{
    AssertUnwindSafe,
    catch_unwind,
};

use qubit_function::Callable;

use super::{
    TaskCompletion,
    TaskExecutionError,
    TaskResult,
};

/// Runner that executes a callable task with standard task-handle semantics.
///
/// `TaskRunner` owns the accepted callable, converts task failures and panics
/// into [`TaskExecutionError`], and can publish the final result through a
/// [`TaskCompletion`] endpoint.
pub struct TaskRunner<C> {
    /// Callable task owned by this runner.
    task: C,
}

impl<C> TaskRunner<C> {
    /// Creates a runner for the supplied callable task.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable task to execute later.
    ///
    /// # Returns
    ///
    /// A runner that owns the callable task.
    #[inline]
    pub const fn new(task: C) -> Self {
        Self { task }
    }

    /// Runs the callable and converts task failure and panic into a handle result.
    ///
    /// # Returns
    ///
    /// `Ok(R)` if the task succeeds. If the task returns `Err(E)` or panics, the
    /// corresponding [`TaskExecutionError`] is returned.
    pub fn call<R, E>(self) -> TaskResult<R, E>
    where
        C: Callable<R, E>,
    {
        let mut task = self.task;
        match catch_unwind(AssertUnwindSafe(|| task.call())) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(err)) => Err(TaskExecutionError::Failed(err)),
            Err(_) => Err(TaskExecutionError::Panicked),
        }
    }

    /// Runs this task through a task completion endpoint.
    ///
    /// # Parameters
    ///
    /// * `completion` - Completion endpoint that publishes the final result.
    ///
    /// # Returns
    ///
    /// `true` if the task started and its result was published, or `false` if
    /// the completion endpoint had already been completed by cancellation.
    #[inline]
    pub fn run<R, E>(self, completion: TaskCompletion<R, E>) -> bool
    where
        C: Callable<R, E>,
    {
        completion.start_and_complete(|| self.call())
    }
}
