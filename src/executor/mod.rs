/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Execution strategy abstractions and basic executor implementations.
//!

mod delay_executor;
mod direct_executor;
#[allow(clippy::module_inception)]
mod executor;
mod schedule_executor;
mod thread_per_task_executor;
mod thread_per_task_executor_builder;
pub(crate) mod thread_spawn_config;

pub use delay_executor::DelayExecutor;
pub use direct_executor::DirectExecutor;
pub use executor::Executor;
pub use schedule_executor::ScheduleExecutor;
pub use thread_per_task_executor::ThreadPerTaskExecutor;
pub use thread_per_task_executor_builder::ThreadPerTaskExecutorBuilder;
