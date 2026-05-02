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
mod future_executor;
mod thread_per_task_executor;

pub use delay_executor::DelayExecutor;
pub use direct_executor::DirectExecutor;
pub use executor::Executor;
pub use future_executor::FutureExecutor;
pub use thread_per_task_executor::ThreadPerTaskExecutor;
