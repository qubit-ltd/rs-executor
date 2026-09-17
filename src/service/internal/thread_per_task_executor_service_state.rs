// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow multiple-public-types

use std::time::Duration;
use std::time::Instant;

use parking_lot::Condvar;
use parking_lot::Mutex;

use super::super::ExecutorServiceLifecycle;
use super::super::SubmissionError;

/// Copyable lifecycle snapshot protected by the service state mutex.
#[derive(Debug, Clone, Copy)]
struct ServiceState {
    /// Current service lifecycle.
    lifecycle: ExecutorServiceLifecycle,
    /// Number of accepted workers that have not exited.
    active_tasks: usize,
}

impl Default for ServiceState {
    /// Creates a running state with no accepted workers.
    fn default() -> Self {
        Self {
            lifecycle: ExecutorServiceLifecycle::Running,
            active_tasks: 0,
        }
    }
}

/// Shared lifecycle and active-task accounting for a thread-per-task service.
#[derive(Default)]
pub struct ThreadPerTaskExecutorServiceState {
    /// Lifecycle snapshot and active worker count.
    state: Mutex<ServiceState>,
    /// Condition variable used by termination waiters.
    termination: Condvar,
}

impl ThreadPerTaskExecutorServiceState {
    /// Returns the current service lifecycle.
    #[inline]
    pub fn lifecycle(&self) -> ExecutorServiceLifecycle {
        self.state.lock().lifecycle
    }

    /// Attempts to admit one task while the service is running.
    ///
    /// # Returns
    ///
    /// `Ok(())` after incrementing the active-task count, or
    /// [`SubmissionError::Shutdown`] when admission is closed.
    #[inline]
    pub fn accept_task(&self) -> Result<(), SubmissionError> {
        let mut state = self.state.lock();
        if state.lifecycle != ExecutorServiceLifecycle::Running {
            return Err(SubmissionError::Shutdown);
        }
        state.active_tasks += 1;
        Ok(())
    }

    /// Decrements the active-task count and terminates the service if ready.
    #[inline]
    pub(crate) fn finish_task(&self) {
        let mut state = self.state.lock();
        state.active_tasks -= 1;
        Self::terminate_if_ready(&mut state, &self.termination);
    }

    /// Blocks until the service reaches the terminated lifecycle.
    pub fn wait_for_termination(&self) {
        let mut state = self.state.lock();
        while state.lifecycle != ExecutorServiceLifecycle::Terminated {
            self.termination.wait(&mut state);
        }
    }

    /// Waits for termination for at most `timeout`.
    ///
    /// # Parameters
    ///
    /// * `timeout` - Maximum monotonic wait duration.
    ///
    /// # Returns
    ///
    /// `true` if termination was observed before the deadline; otherwise
    /// `false`.
    pub fn wait_for_termination_timeout(&self, timeout: Duration) -> bool {
        let started = Instant::now();
        let mut state = self.state.lock();
        loop {
            if state.lifecycle == ExecutorServiceLifecycle::Terminated {
                return true;
            }
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return false;
            }
            self.termination
                .wait_for(&mut state, remaining.min(Duration::from_secs(3600)));
        }
    }

    /// Requests graceful shutdown and terminates immediately if idle.
    #[inline]
    pub fn shutdown(&self) {
        let mut state = self.state.lock();
        if state.lifecycle == ExecutorServiceLifecycle::Running {
            state.lifecycle = ExecutorServiceLifecycle::ShuttingDown;
        }
        Self::terminate_if_ready(&mut state, &self.termination);
    }

    /// Requests immediate stop and returns the active-task count observed.
    ///
    /// # Returns
    ///
    /// The number of accepted workers still active when stop was requested.
    #[inline]
    pub fn stop(&self) -> usize {
        let mut state = self.state.lock();
        if state.lifecycle != ExecutorServiceLifecycle::Terminated {
            state.lifecycle = ExecutorServiceLifecycle::Stopping;
        }
        let running = state.active_tasks;
        Self::terminate_if_ready(&mut state, &self.termination);
        running
    }

    /// Marks the service terminated when no active work remains.
    #[inline]
    fn terminate_if_ready(state: &mut ServiceState, termination: &Condvar) {
        if state.lifecycle != ExecutorServiceLifecycle::Running && state.active_tasks == 0 {
            state.lifecycle = ExecutorServiceLifecycle::Terminated;
            termination.notify_all();
        }
    }
}
