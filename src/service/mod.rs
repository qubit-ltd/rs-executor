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

mod executor_service;
mod rejected_execution;
mod shutdown_report;
mod thread_per_task_executor_service;

pub use executor_service::ExecutorService;
pub use rejected_execution::RejectedExecution;
pub use shutdown_report::ShutdownReport;
pub use thread_per_task_executor_service::ThreadPerTaskExecutorService;
