// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::super::scheduler_core::SchedulerCore;

/// Owns the active stop call's drain of detached scheduled entries.
///
/// Only the stop call that sets `stop_draining` for a non-empty batch creates
/// this guard. Dropping it on normal scope exit or during unwinding clears
/// `stop_draining` while holding the scheduler state lock, publishes
/// `terminated` only when `SchedulerState::can_terminate()` allows it, and
/// notifies state waiters. It does not interrupt an active task; that task's
/// worker guard must clear `worker_active` before termination can be published.
pub(crate) struct StopDrainGuard<'a> {
    /// Scheduler core whose state lock protects the drain this guard owns.
    core: &'a SchedulerCore,
}

impl<'a> StopDrainGuard<'a> {
    /// Creates the unique guard for the stop call draining a non-empty batch.
    pub(crate) fn new(core: &'a SchedulerCore) -> Self {
        Self { core }
    }
}

impl Drop for StopDrainGuard<'_> {
    /// Clears the owned drain and conditionally publishes termination under
    /// the state lock.
    fn drop(&mut self) {
        self.core.state.with_write_notify_all(|state| {
            state.stop_draining = false;
            if state.can_terminate() {
                state.terminated = true;
            }
        });
    }
}
