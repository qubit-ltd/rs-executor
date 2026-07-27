// =============================================================================
// qubit-style: allow source-test-pair
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::{sync::Arc, time::Instant};

use super::{scheduled_task_entry::StartedScheduledTask, scheduler_core::SchedulerCore};

/// Worker loop entry point for single-thread scheduled executor services.
pub(crate) struct ScheduledWorker;

impl ScheduledWorker {
    /// Runs the scheduled executor service loop.
    pub(crate) fn run(core: Arc<SchedulerCore>) {
        loop {
            let Some(task) = next_ready_task(&core) else {
                return;
            };
            task();
            finish_task(&core);
        }
    }
}

/// Obtains the next due task without retaining the monitor during task work.
fn next_ready_task(core: &SchedulerCore) -> Option<StartedScheduledTask> {
    let mut state = core.state.lock();
    loop {
        if state.can_terminate() {
            state.terminated = true;
            core.state.notify_all();
            return None;
        }
        if state.stop_draining {
            state.wait();
            continue;
        }
        let Some(first) = state.tasks.first() else {
            state.wait();
            continue;
        };
        let deadline = *first.order();
        let now = Instant::now();
        if deadline > now {
            let _ = state
                .wait_for(deadline.saturating_duration_since(now))
                .expect("scheduler worker waiter should remain registered");
            continue;
        }
        let entry = state
            .tasks
            .pop_first()
            .expect("observed first scheduled entry must remain present");
        state.worker_active = true;
        drop(state);
        return entry.into_value().start();
    }
}

/// Records that the worker released its active entry.
fn finish_task(core: &SchedulerCore) {
    core.state.with_write_notify_all(|state| {
        state.worker_active = false;
        if state.can_terminate() {
            state.terminated = true;
        }
    });
}
