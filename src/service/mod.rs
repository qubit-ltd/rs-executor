/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Managed executor service abstractions and basic service implementations.
//!
//! # Author
//!
//! Haixing Hu

mod executor_service;
mod rejected_execution;
mod shutdown_report;
mod thread_per_task_executor_service;

pub use executor_service::ExecutorService;
pub use rejected_execution::RejectedExecution;
pub use shutdown_report::ShutdownReport;
pub use thread_per_task_executor_service::ThreadPerTaskExecutorService;
