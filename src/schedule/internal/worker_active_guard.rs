// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::super::scheduler_core::SchedulerCore;

/// Owns cleanup of the active-worker marker for one claimed scheduled entry.
///
/// Create this guard only after the scheduler has set `worker_active` to
/// `true` for the claimed entry. Dropping it on normal scope exit or during
/// unwinding clears that marker while holding the scheduler state lock,
/// publishes `terminated` only when `SchedulerState::can_terminate()` allows
/// it, and notifies state waiters. This cleanup also applies when a recoverable
/// unwind occurs while starting or running the entry.
pub(crate) struct WorkerActiveGuard<'a> {
    /// Scheduler core whose state lock protects the active marker owned here.
    core: &'a SchedulerCore,
}

impl<'a> WorkerActiveGuard<'a> {
    /// Creates a guard for an entry already marked active by the scheduler.
    pub(crate) fn new(core: &'a SchedulerCore) -> Self {
        Self { core }
    }
}

impl Drop for WorkerActiveGuard<'_> {
    /// Clears the owned active marker and conditionally publishes termination
    /// under the state lock.
    fn drop(&mut self) {
        self.core.state.with_write_notify_all(|state| {
            state.worker_active = false;
            if state.can_terminate() {
                state.terminated = true;
            }
        });
    }
}
