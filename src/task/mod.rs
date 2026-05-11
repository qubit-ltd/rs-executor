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

pub mod cancel_result;
pub mod task_completion;
pub mod task_completion_pair;
pub mod task_execution_error;
pub mod task_handle;
pub mod task_handle_future;
pub mod task_handle_inner;
pub mod task_result_handle;
mod task_runner;
pub mod task_status;
pub mod tracked_task;
pub mod tracked_task_handle;
pub mod try_get;

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
