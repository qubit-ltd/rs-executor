// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Scheduled executor service abstractions and basic implementations.

mod completable_scheduled_task;
mod scheduled_executor_service;
mod scheduled_task_entry;
mod scheduled_task_handle;
mod scheduled_worker;
mod scheduler_core;
mod scheduler_state;
mod single_thread_scheduled_executor_service;

pub use scheduled_executor_service::ScheduledExecutorService;
pub use scheduled_task_handle::ScheduledTaskHandle;
pub use single_thread_scheduled_executor_service::SingleThreadScheduledExecutorService;
