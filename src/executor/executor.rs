/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use qubit_function::{
    Callable,
    Runnable,
};

use crate::service::SubmissionError;

/// Executes fallible one-time tasks according to an implementation-defined strategy.
///
/// `Executor` models an execution strategy, not a managed task service. An
/// executor may run a task immediately, retry it, delay it, schedule it on
/// another runtime, or return a handle that represents work running elsewhere.
/// The associated [`Self::Output`] type describes how this executor exposes the
/// accepted task's result. The outer `Result` returned by [`Self::call`] and
/// [`Self::execute`] always reports submission failure only.
///
pub trait Executor: Send + Sync {
    /// The result carrier returned for one accepted execution.
    ///
    /// Implementations choose the carrier that matches their execution model.
    /// For example, a direct executor can use a ready task result, while a
    /// threaded executor can use a task handle.
    type Output<R, E>
    where
        R: Send + 'static,
        E: Send + 'static;

    /// Submits a runnable task and returns this executor's accepted-task output.
    ///
    /// This is the unit-returning counterpart of [`Self::call`]. The returned
    /// carrier reports the runnable's `Result<(), E>` according to the concrete
    /// executor's execution model.
    ///
    /// # Parameters
    ///
    /// * `task` - The fallible action to execute.
    ///
    /// # Returns
    ///
    /// The accepted-task output for the submitted runnable.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError`] if this executor cannot accept the runnable.
    #[inline]
    fn execute<T, E>(&self, task: T) -> Result<Self::Output<(), E>, SubmissionError>
    where
        T: Runnable<E> + Send + 'static,
        E: Send + 'static,
    {
        let mut task = task;
        self.call(move || task.run())
    }

    /// Submits a callable task and returns this executor's accepted-task output.
    ///
    /// # Parameters
    ///
    /// * `task` - The fallible computation to execute.
    ///
    /// # Returns
    ///
    /// The accepted-task output for the submitted callable. Its exact behavior is
    /// defined by the concrete executor.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError`] if this executor cannot accept the callable.
    fn call<C, R, E>(&self, task: C) -> Result<Self::Output<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static;
}
