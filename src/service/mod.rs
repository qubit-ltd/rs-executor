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
mod executor_service_builder_error;
mod executor_service_lifecycle;
mod stop_report;
mod submission_error;
mod thread_per_task_executor_service;
mod thread_per_task_executor_service_builder;

pub use executor_service::ExecutorService;
pub use executor_service_builder_error::ExecutorServiceBuilderError;
pub use executor_service_lifecycle::ExecutorServiceLifecycle;
pub use stop_report::StopReport;
pub use submission_error::SubmissionError;
pub use thread_per_task_executor_service::ThreadPerTaskExecutorService;
pub use thread_per_task_executor_service_builder::ThreadPerTaskExecutorServiceBuilder;
