// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! # Qubit Executor
//!
//! Core executor abstractions, task handles, and basic executor
//! implementations.

pub mod executor;
pub mod hook;
pub mod schedule;
pub mod service;
pub mod task;

pub use crate::executor::DelayExecutor;
pub use crate::executor::DirectExecutor;
pub use crate::executor::Executor;
pub use crate::executor::ScheduleExecutor;
pub use crate::executor::ThreadPerTaskExecutor;
pub use crate::executor::ThreadPerTaskExecutorBuilder;
pub use crate::schedule::ScheduledExecutorService;
pub use crate::schedule::ScheduledTaskHandle;
pub use crate::schedule::SingleThreadScheduledExecutorService;
pub use crate::service::ExecutorService;
pub use crate::service::ExecutorServiceBuilderError;
pub use crate::service::ExecutorServiceLifecycle;
pub use crate::service::StopReport;
pub use crate::service::SubmissionError;
pub use crate::service::ThreadPerTaskExecutorService;
pub use crate::service::ThreadPerTaskExecutorServiceBuilder;
pub use crate::task::CancelResult;
pub use crate::task::TaskExecutionError;
pub use crate::task::TaskHandle;
pub use crate::task::TaskResult;
pub use crate::task::TaskStatus;
pub use crate::task::TrackedTask;
pub use crate::task::TryGet;
