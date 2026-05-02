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

pub mod task_completion;
pub mod task_completion_pair;
pub mod task_execution_error;
pub mod task_handle;
pub mod task_handle_inner;
pub mod task_handle_state;
mod task_runner;

pub use task_completion::TaskCompletion;
pub use task_completion_pair::TaskCompletionPair;
pub use task_execution_error::{
    TaskExecutionError,
    TaskResult,
};
pub use task_handle::TaskHandle;
pub use task_runner::TaskRunner;
