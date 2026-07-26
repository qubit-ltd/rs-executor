// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Task lifecycle hooks.

mod logging_task_hook;
mod noop_task_hook;
mod task_hook;
mod task_id;

pub use logging_task_hook::LoggingTaskHook;
pub use noop_task_hook::NoopTaskHook;
pub use task_hook::TaskHook;
pub use task_id::TaskId;

pub(crate) use task_hook::{
    notify_accepted, notify_finished, notify_rejected, notify_rejected_optional, notify_started,
};
pub(crate) use task_id::next_task_id;
