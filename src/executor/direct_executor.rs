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
    TrackedTask,
    service::SubmissionError,
    task::spi::{
        TaskEndpointPair,
        TaskRunner,
    },
};

use super::Executor;

/// Executes tasks immediately on the caller thread.
///
/// This executor is useful for deterministic tests and simple composition
/// where task execution should happen in the same call stack.
#[derive(Debug, Default, Clone, Copy)]
pub struct DirectExecutor;

impl Executor for DirectExecutor {
    /// Executes the callable inline and returns an already completed handle.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to run on the caller thread.
    ///
    /// # Returns
    ///
    /// An already completed tracked task carrying the callable result.
    #[inline]
    fn call<C, R, E>(&self, task: C) -> Result<TrackedTask<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let (handle, completion) = TaskEndpointPair::new().into_tracked_parts();
        TaskRunner::new(task).run(completion);
        Ok(handle)
    }
}
