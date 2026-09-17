// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;

use super::thread_per_task_executor_service_state::ThreadPerTaskExecutorServiceState;

/// Decrements service active-task accounting when a worker exits.
pub struct ActiveTaskGuard {
    /// Shared service state whose active-task count this guard owns.
    state: Arc<ThreadPerTaskExecutorServiceState>,
}

impl ActiveTaskGuard {
    /// Creates an active-task guard for one accepted worker.
    ///
    /// # Parameters
    ///
    /// * `state` - Shared state containing the active-task counter.
    ///
    /// # Returns
    ///
    /// A guard that marks one task finished when dropped.
    #[inline]
    pub fn new(state: Arc<ThreadPerTaskExecutorServiceState>) -> Self {
        Self { state }
    }
}

impl Drop for ActiveTaskGuard {
    /// Decrements the active-task count and wakes termination waiters as
    /// needed.
    #[inline]
    fn drop(&mut self) {
        self.state.finish_task();
    }
}
