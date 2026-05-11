/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use qubit_function::Callable;

use crate::{
    TaskResult,
    service::SubmissionError,
    task::TaskRunner,
};

use super::Executor;

/// Executes tasks immediately on the caller thread.
///
/// This executor is useful for deterministic tests and simple composition
/// where task execution should happen in the same call stack.
#[derive(Debug, Default, Clone, Copy)]
pub struct DirectExecutor;

impl Executor for DirectExecutor {
    type Output<R, E>
        = TaskResult<R, E>
    where
        R: Send + 'static,
        E: Send + 'static;

    /// Executes the callable inline and returns its result.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to run on the caller thread.
    ///
    /// # Returns
    ///
    /// The ready task result produced by the callable.
    #[inline]
    fn call<C, R, E>(&self, task: C) -> Result<Self::Output<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        Ok(TaskRunner::new(task).call())
    }
}
