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

/// Runs a callable and converts task failure and panic into a handle result.
pub fn run_callable<C, R, E>(mut task: C) -> TaskResult<R, E>
where
    C: Callable<R, E>,
{
    match catch_unwind(AssertUnwindSafe(|| task.call())) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => Err(TaskExecutionError::Failed(err)),
        Err(_) => Err(TaskExecutionError::Panicked),
    }
}

/// Runs a callable task through a task completion endpoint.
pub fn run_task<C, R, E>(task: C, completion: TaskCompletion<R, E>)
where
    C: Callable<R, E>,
{
    completion.start_and_complete(|| run_callable(task));
}
