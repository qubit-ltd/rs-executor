/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! # Qubit Executor
//!
//! Core executor abstractions, task handles, and basic executor implementations.
//!

pub mod executor;
pub mod hook;
pub mod schedule;
pub mod service;
pub mod task;

pub use crate::executor::{
    DelayExecutor,
    DirectExecutor,
    Executor,
    ScheduleExecutor,
    ThreadPerTaskExecutor,
    ThreadPerTaskExecutorBuilder,
};
pub use crate::schedule::{
    ScheduledExecutorService,
    ScheduledTaskHandle,
    SingleThreadScheduledExecutorService,
};
pub use crate::service::{
    ExecutorService,
    ExecutorServiceBuilderError,
    ExecutorServiceLifecycle,
    StopReport,
    SubmissionError,
    ThreadPerTaskExecutorService,
    ThreadPerTaskExecutorServiceBuilder,
};
pub use crate::task::{
    CancelResult,
    TaskExecutionError,
    TaskResult,
    TaskStatus,
};
pub use crate::task::{
    TaskHandle,
    TrackedTask,
    TryGet,
};
