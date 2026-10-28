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
#[must_use = "the guard must be kept alive until the worker exits"]
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::super::ThreadPerTaskExecutorServiceState;
    use super::ActiveTaskGuard;
    use crate::ExecutorServiceLifecycle;

    #[test]
    fn test_service_admission_guard_rollback_after_shutdown() {
        let state = Arc::new(ThreadPerTaskExecutorServiceState::default());
        state.accept_task().expect("admit before close");
        let guard = ActiveTaskGuard::new(Arc::clone(&state));
        std::thread::scope(|scope| {
            scope.spawn(|| state.shutdown()).join().expect("closer returns");
        });
        let before_drop = state.lifecycle();
        drop(guard);
        assert_eq!(before_drop, ExecutorServiceLifecycle::ShuttingDown);
        assert!(state.wait_for_termination_timeout(Duration::from_secs(5)));
        assert_eq!(state.lifecycle(), ExecutorServiceLifecycle::Terminated);
    }

    #[test]
    fn test_service_admission_guard_rollback_after_stop() {
        let state = Arc::new(ThreadPerTaskExecutorServiceState::default());
        state.accept_task().expect("admit before close");
        let guard = ActiveTaskGuard::new(Arc::clone(&state));
        std::thread::scope(|scope| {
            scope.spawn(|| state.stop()).join().expect("closer returns");
        });
        let before_drop = state.lifecycle();
        drop(guard);
        assert_eq!(before_drop, ExecutorServiceLifecycle::Stopping);
        assert!(state.wait_for_termination_timeout(Duration::from_secs(5)));
        assert_eq!(state.lifecycle(), ExecutorServiceLifecycle::Terminated);
    }
}
