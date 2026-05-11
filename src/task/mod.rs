/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Task-related internal modules.
//!
//! This module groups task handle, execution error, and runner utilities that
//! used to live at the crate root. They are reorganized under `task/`.

mod cancel_result;
mod task_completion;
mod task_completion_pair;
mod task_execution_error;
mod task_handle;
mod task_handle_future;
mod task_handle_inner;
mod task_result_handle;
mod task_runner;
mod task_status;
mod tracked_task;
mod tracked_task_handle;
mod try_get;

pub use cancel_result::CancelResult;
pub use task_completion::TaskCompletion;
pub use task_completion_pair::TaskCompletionPair;
pub use task_execution_error::{
    TaskExecutionError,
    TaskResult,
};
pub use task_handle::TaskHandle;
pub use task_handle_future::TaskHandleFuture;
pub use task_result_handle::TaskResultHandle;
pub use task_runner::TaskRunner;
pub use task_status::TaskStatus;
pub use tracked_task::TrackedTask;
pub use tracked_task_handle::TrackedTaskHandle;
pub use try_get::TryGet;
