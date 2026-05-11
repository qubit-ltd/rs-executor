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

use crate::{
    TrackedTask,
    service::SubmissionError,
};

/// Executes fallible one-time tasks according to an implementation-defined strategy.
///
/// `Executor` models an execution strategy, not a managed task service. An
/// executor may run a task immediately, retry it, delay it, or schedule it on
/// another runtime. Accepted task results are always exposed through a
/// [`TrackedTask`]. The outer `Result` returned by [`Self::call`] and
/// [`Self::execute`] reports submission failure only.
///
pub trait Executor: Send + Sync {
    /// Submits a runnable task and returns a tracked task handle.
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
    /// A tracked handle for the accepted runnable.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError`] if this executor cannot accept the runnable.
    #[inline]
    fn execute<T, E>(&self, task: T) -> Result<TrackedTask<(), E>, SubmissionError>
    where
        T: Runnable<E> + Send + 'static,
        E: Send + 'static,
    {
        let mut task = task;
        self.call(move || task.run())
    }

    /// Submits a callable task and returns a tracked task handle.
    ///
    /// # Parameters
    ///
    /// * `task` - The fallible computation to execute.
    ///
    /// # Returns
    ///
    /// A tracked handle for the accepted callable.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError`] if this executor cannot accept the callable.
    fn call<C, R, E>(&self, task: C) -> Result<TrackedTask<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static;
}
