// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::super::scheduler_core::SchedulerCore;

/// Clears the active-worker marker after one scheduled entry finishes.
pub(crate) struct WorkerActiveGuard<'a> {
    core: &'a SchedulerCore,
}

impl<'a> WorkerActiveGuard<'a> {
    /// Creates a guard for an entry already marked active by the scheduler.
    pub(crate) fn new(core: &'a SchedulerCore) -> Self {
        Self { core }
    }
}

impl Drop for WorkerActiveGuard<'_> {
    fn drop(&mut self) {
        self.core.state.with_write_notify_all(|state| {
            state.worker_active = false;
            if state.can_terminate() {
                state.terminated = true;
            }
        });
    }
}
