// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::{sync::Arc, time::Instant};

use crate::service::ExecutorServiceLifecycle;

use super::{
    scheduled_task_entry::StartedScheduledTask,
    single_thread_scheduled_executor_service_inner::SingleThreadScheduledExecutorServiceInner,
    single_thread_scheduled_executor_service_state::SingleThreadScheduledExecutorServiceState,
};

/// Worker loop entry point for single-thread scheduled executor services.
pub(crate) struct ScheduledWorker;

impl ScheduledWorker {
    /// Runs the scheduled executor service loop.
    ///
    /// # Parameters
    ///
    /// * `inner` - Shared scheduler state.
    pub(crate) fn run(inner: Arc<SingleThreadScheduledExecutorServiceInner>) {
        run_scheduled_worker(inner);
    }
}

/// Runs the scheduled executor service loop.
///
/// # Parameters
///
/// * `inner` - Shared scheduler state.
fn run_scheduled_worker(inner: Arc<SingleThreadScheduledExecutorServiceInner>) {
    loop {
        let task = next_ready_task(&inner);
        let Some(task) = task else {
            return;
        };
        task();
        inner.finish_running_task();
    }
}

/// Waits until a scheduled task is ready or the worker should terminate.
///
/// # Parameters
///
/// * `inner` - Shared scheduler state.
///
/// # Returns
///
/// A started task closure to run outside the scheduler lock, or `None` when the
/// worker should terminate.
fn next_ready_task(
    inner: &SingleThreadScheduledExecutorServiceInner,
) -> Option<StartedScheduledTask> {
    let mut state = inner.state.lock();
    loop {
        prune_cancelled_front(&mut state);
        if state.lifecycle == ExecutorServiceLifecycle::Stopping {
            inner.terminate(&mut state);
            return None;
        }
        if state.tasks.is_empty() && state.lifecycle != ExecutorServiceLifecycle::Running {
            inner.terminate(&mut state);
            return None;
        }
        let Some(next_deadline) = state.tasks.peek().map(|task| task.deadline) else {
            state.wait();
            continue;
        };
        let now = Instant::now();
        if next_deadline > now {
            let timeout = next_deadline.saturating_duration_since(now);
            let _status = state
                .wait_for(timeout)
                .expect("standard Timer should register");
            continue;
        }
        let Some(task) = state.tasks.pop() else {
            continue;
        };
        let Some(started_task) = task.entry.start() else {
            continue;
        };
        state.start_task();
        return Some(started_task);
    }
}

/// Removes already-cancelled tasks from the front of the deadline heap.
///
/// # Parameters
///
/// * `state` - Locked scheduler state.
fn prune_cancelled_front(state: &mut SingleThreadScheduledExecutorServiceState) {
    while state
        .tasks
        .peek()
        .is_some_and(|task| task.entry.is_cancelled())
    {
        state.tasks.pop();
    }
}
