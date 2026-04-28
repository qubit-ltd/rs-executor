/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
use qubit_function::Callable;

use super::Executor;

/// Executes tasks immediately on the caller thread.
///
/// This executor is useful for deterministic tests and simple composition
/// where task execution should happen in the same call stack.
#[derive(Debug, Default, Clone, Copy)]
pub struct DirectExecutor;

impl Executor for DirectExecutor {
    type Execution<R, E>
        = Result<R, E>
    where
        R: Send + 'static,
        E: std::fmt::Display + Send + 'static;

    /// Executes the callable inline and returns its result.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to run on the caller thread.
    ///
    /// # Returns
    ///
    /// The exact `Result<R, E>` returned by the callable.
    #[inline]
    fn call<C, R, E>(&self, mut task: C) -> Self::Execution<R, E>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        task.call()
    }
}
