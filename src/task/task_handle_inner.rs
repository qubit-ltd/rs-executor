/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use qubit_atomic::Atomic;
use qubit_lock::Monitor;

use super::task_handle_state::TaskHandleState;

/// Shared state used by a task handle and its completing runner.
pub(crate) struct TaskHandleInner<R, E> {
    /// Monitor protecting the task result, state flags, and async waker.
    pub(crate) state: Monitor<TaskHandleState<R, E>>,
    /// Atomic completion flag for cheap non-blocking probes.
    pub(crate) done: Atomic<bool>,
}

impl<R, E> TaskHandleInner<R, E> {
    /// Notifies every waiter that the shared task state may have changed.
    ///
    /// This wakes blocking waiters parked in [`Monitor::wait_until`].
    #[inline]
    pub(crate) fn notify_completion(&self) {
        self.state.notify_all();
    }
}
