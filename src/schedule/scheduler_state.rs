// =============================================================================
// qubit-style: allow source-test-pair
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::time::Instant;

use qubit_collections::map::OrderedIndexMap;

use crate::{hook::TaskId, service::ExecutorServiceLifecycle};

use super::scheduled_task_entry::ScheduledTaskEntry;

/// Mutable state protected by the scheduler monitor.
pub(crate) struct SchedulerState {
    /// Current service lifecycle.
    pub(crate) lifecycle: ExecutorServiceLifecycle,
    /// Tasks addressable by id and ordered stably by deadline.
    pub(crate) tasks: OrderedIndexMap<TaskId, Instant, Box<dyn ScheduledTaskEntry>>,
    /// Whether the worker has exited.
    pub(crate) terminated: bool,
    /// Whether the worker owns an entry outside the monitor.
    pub(crate) worker_active: bool,
    /// Whether stop is cancelling detached entries outside the monitor.
    pub(crate) stop_draining: bool,
}

impl SchedulerState {
    /// Creates an empty running scheduler state.
    pub(crate) fn new() -> Self {
        Self {
            lifecycle: ExecutorServiceLifecycle::Running,
            tasks: OrderedIndexMap::new(),
            terminated: false,
            worker_active: false,
            stop_draining: false,
        }
    }

    /// Returns whether no entry or worker activity remains.
    pub(crate) fn can_terminate(&self) -> bool {
        self.lifecycle != ExecutorServiceLifecycle::Running
            && self.tasks.is_empty()
            && !self.worker_active
            && !self.stop_draining
    }
}
