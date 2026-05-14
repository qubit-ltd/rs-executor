/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Task handles and task-result types.

mod atomic_task_status;
mod cancel_result;
mod running_task_slot;
pub(crate) mod task_admission_gate;
mod task_endpoint_pair;
mod task_execution_error;
mod task_handle;
mod task_handle_future;
mod task_result_handle;
mod task_runner;
mod task_slot;
mod task_state;
mod task_status;
mod tracked_task;
mod tracked_task_handle;
mod try_get;

pub use cancel_result::CancelResult;
pub use task_execution_error::{
    TaskExecutionError,
    TaskResult,
};
pub use task_handle::TaskHandle;
pub use task_handle_future::TaskHandleFuture;
pub use task_status::TaskStatus;
pub use tracked_task::TrackedTask;
pub use try_get::TryGet;

/// Service-provider interfaces for custom executor implementations.
///
/// These low-level building blocks are intended for crates that implement
/// executor services or execution strategies. Ordinary users should prefer
/// [`TaskHandle`], [`TrackedTask`], and the executor/service traits.
pub mod spi {
    pub use super::running_task_slot::RunningTaskSlot;
    pub use super::task_endpoint_pair::TaskEndpointPair;
    pub use super::task_result_handle::TaskResultHandle;
    pub use super::task_runner::TaskRunner;
    pub use super::task_slot::TaskSlot;
    pub use super::tracked_task_handle::TrackedTaskHandle;
}
