// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! # Qubit Executor
//!
//! Runtime-independent abstractions for executing tasks and observing their
//! results. Use [`Executor`] to choose how an individual task runs, and
//! [`ExecutorService`] when accepted work needs managed shutdown.
//! [`TrackedTask`] provides access to a submitted task's completion and result.
//!
//! A direct executor runs a task on the calling thread:
//!
//! ```
//! use qubit_executor::DirectExecutor;
//! use qubit_executor::Executor;
//!
//! let executor = DirectExecutor::new();
//! let task = executor.call(|| Ok::<usize, ()>(42)).expect("task accepted");
//! assert_eq!(task.get().expect("task completed"), 42);
//! ```
//!
//! A service manages accepted work and can be shut down after its results are
//! observed:
//!
//! ```
//! use qubit_executor::ExecutorService;
//! use qubit_executor::ThreadPerTaskExecutorService;
//!
//! let service = ThreadPerTaskExecutorService::new();
//! let task = service.submit_callable(|| Ok::<usize, ()>(42)).expect("task accepted");
//! assert_eq!(task.get().expect("task completed"), 42);
//! service.shutdown();
//! service.wait_termination();
//! ```
//!
//! Successful submission means that work was accepted, not that it has
//! completed. Calling `get()` blocks the calling thread until the task result
//! is available. Cancellation cannot forcibly interrupt arbitrary running Rust
//! code, and service termination does not join every OS thread or guarantee
//! that all external resources have been released.
//!
//! # User guides
//!
//! See the [English user guide](https://github.com/qubit-ltd/rs-executor/blob/main/doc/user_guide.md)
//! and the [简体中文用户指南](https://github.com/qubit-ltd/rs-executor/blob/main/doc/user_guide.zh_CN.md).

mod internal;

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
