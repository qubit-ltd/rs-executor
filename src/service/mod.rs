/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Managed executor service abstractions and basic service implementations.
//!

mod executor_build_error;
mod executor_service;
mod executor_service_lifecycle;
mod rejected_execution;
mod stop_report;
mod thread_per_task_executor_service;

pub use executor_build_error::ExecutorBuildError;
pub use executor_service::ExecutorService;
pub use executor_service_lifecycle::ExecutorServiceLifecycle;
pub use rejected_execution::RejectedExecution;
pub use stop_report::StopReport;
pub use thread_per_task_executor_service::ThreadPerTaskExecutorService;
