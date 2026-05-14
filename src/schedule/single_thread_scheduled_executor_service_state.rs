/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::collections::BinaryHeap;

use crate::service::ExecutorServiceLifecycle;

use super::scheduled_task::ScheduledTask;

/// Mutable scheduler state protected by the service monitor.
pub(crate) struct SingleThreadScheduledExecutorServiceState {
    /// Current lifecycle state.
    pub(crate) lifecycle: ExecutorServiceLifecycle,
    /// Deadline-ordered task heap.
    pub(crate) tasks: BinaryHeap<ScheduledTask>,
    /// Sequence used to keep stable order for identical deadlines.
    pub(crate) next_sequence: usize,
    /// Whether the scheduler thread has exited.
    pub(crate) terminated: bool,
}

impl SingleThreadScheduledExecutorServiceState {
    /// Creates an empty running scheduler state.
    ///
    /// # Returns
    ///
    /// A running state with no scheduled tasks.
    pub(crate) fn new() -> Self {
        Self {
            lifecycle: ExecutorServiceLifecycle::Running,
            tasks: BinaryHeap::new(),
            next_sequence: 0,
            terminated: false,
        }
    }
}
