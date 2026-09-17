// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Internal helpers for the thread-per-task executor service.

mod active_task_guard;
mod task_admission_handle;
mod thread_per_task_executor_service_state;

pub(super) use active_task_guard::ActiveTaskGuard;
pub(super) use task_admission_handle::TaskAdmissionHandle;
pub(super) use thread_per_task_executor_service_state::ThreadPerTaskExecutorServiceState;
