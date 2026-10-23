// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;

use qubit_function::Callable;

use super::TaskExecutionError;
use super::TaskResult;
use super::running_task_slot::RunningTaskSlot;
use super::task_slot::TaskSlot;

/// Runner that executes a callable task with standard task-handle semantics.
///
/// `TaskRunner` owns the accepted callable, converts task failures and panics
/// into [`TaskExecutionError`], and can publish the final result through a
/// [`TaskSlot`] endpoint.
///
/// # Type Parameters
///
/// * `C` - Callable task that produces the task result.
///
/// # Examples
///
/// ```
/// use qubit_executor::task::spi::TaskRunner;
///
/// let result = TaskRunner::new(|| Ok::<usize, std::convert::Infallible>(42))
///     .call::<usize, std::convert::Infallible>();
/// assert!(matches!(result, Ok(42)));
/// ```
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
    ///
    /// # Type Parameters
    ///
    /// * `C` - Callable task type.
    #[inline]
    pub const fn new(task: C) -> Self {
        Self { task }
    }

    /// Runs the callable and converts task failure and panic into a handle
    /// result.
    ///
    /// # Returns
    ///
    /// `Ok(R)` if the task succeeds. If the task returns `Err(E)` or panics,
    /// the corresponding [`TaskExecutionError`] is returned.
    ///
    /// # Errors
    ///
    /// Returns [`TaskExecutionError::Failed`] when the task returns an error,
    /// or [`TaskExecutionError::Panicked`] when the task panics.
    ///
    /// # Type Parameters
    ///
    /// * `R` - Value produced by a successful task.
    /// * `E` - Error returned by the task.
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
    /// the completion endpoint had already been completed before the runner
    /// started.
    ///
    /// # Type Parameters
    ///
    /// * `R` - Value produced by a successful task.
    /// * `E` - Error returned by the task.
    #[inline]
    pub fn run<R, E>(self, slot: TaskSlot<R, E>) -> bool
    where
        C: Callable<R, E>,
    {
        slot.start_and_complete(|| self.call())
    }

    /// Runs this task through an already-started task slot.
    ///
    /// # Parameters
    ///
    /// * `slot` - Running slot that has already won the pending-to-running
    ///   transition.
    ///
    /// # Returns
    ///
    /// `true` if the task result was published, or `false` if completion was
    /// no longer possible.
    ///
    /// # Type Parameters
    ///
    /// * `R` - Value produced by a successful task.
    /// * `E` - Error returned by the task.
    #[inline]
    pub fn run_started<R, E>(self, slot: RunningTaskSlot<R, E>) -> bool
    where
        C: Callable<R, E>,
    {
        slot.complete(self.call())
    }

    /// Runs this task and discards its final result.
    ///
    /// This is intended for detached runnable submissions whose caller only
    /// observes acceptance. The callable is still wrapped in panic conversion,
    /// but success, task failure, and panic results are intentionally dropped.
    ///
    /// # Type Parameters
    ///
    /// * `R` - Value produced by a successful task and discarded after
    ///   execution.
    /// * `E` - Error returned by the task and discarded after execution.
    pub fn run_detached<R, E>(self)
    where
        C: Callable<R, E>,
    {
        let _ignored = self.call::<R, E>();
    }
}
