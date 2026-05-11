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

use super::task_status::TaskStatus;

/// Atomic state machine for one tracked task status.
pub(crate) struct AtomicTaskStatus {
    /// Compact atomic representation of the current task status.
    value: Atomic<u8>,
}

impl AtomicTaskStatus {
    /// Creates an atomic task status initialized with the supplied status.
    ///
    /// # Parameters
    ///
    /// * `status` - Initial task status.
    ///
    /// # Returns
    ///
    /// A task status cell initialized to `status`.
    #[inline]
    pub(crate) fn new(status: TaskStatus) -> Self {
        Self {
            value: Atomic::new(status.as_u8()),
        }
    }

    /// Loads the current task status.
    ///
    /// # Returns
    ///
    /// The currently observed task status.
    #[inline]
    pub(crate) fn load(&self) -> TaskStatus {
        TaskStatus::from_u8(self.value.load())
    }

    /// Attempts to move a pending task into running state.
    ///
    /// # Returns
    ///
    /// `true` if the state changed from pending to running.
    #[inline]
    pub(crate) fn start(&self) -> bool {
        self.compare_set(TaskStatus::Pending, TaskStatus::Running)
    }

    /// Attempts to move the task from `current` to `next`.
    ///
    /// # Parameters
    ///
    /// * `current` - Expected current status.
    /// * `next` - Desired next status.
    ///
    /// # Returns
    ///
    /// `true` if the compare-and-set operation succeeded.
    #[inline]
    pub(crate) fn compare_set(&self, current: TaskStatus, next: TaskStatus) -> bool {
        self.value
            .compare_set(current.as_u8(), next.as_u8())
            .is_ok()
    }
}
