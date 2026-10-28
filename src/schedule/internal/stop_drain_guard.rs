// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::super::scheduler_core::SchedulerCore;

/// Restores the scheduler drain state when a stop call unwinds.
///
/// This guard represents the unique stop call that owns the current drain.
pub(crate) struct StopDrainGuard<'a> {
    core: &'a SchedulerCore,
}

impl<'a> StopDrainGuard<'a> {
    /// Creates a guard for the stop call that owns a non-empty drain batch.
    pub(crate) fn new(core: &'a SchedulerCore) -> Self {
        Self { core }
    }
}

impl Drop for StopDrainGuard<'_> {
    fn drop(&mut self) {
        self.core.state.with_write_notify_all(|state| {
            state.stop_draining = false;
            if state.can_terminate() {
                state.terminated = true;
            }
        });
    }
}
