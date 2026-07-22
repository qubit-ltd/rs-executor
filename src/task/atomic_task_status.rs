// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fast_cas::FastCasState;

use super::task_status::TaskStatus;
use super::task_status_event::TaskStatusEvent;
use super::task_status_machine::TASK_STATUS_MACHINE;

/// Atomic state machine for one tracked task status.
pub(crate) struct AtomicTaskStatus {
    /// Compact atomic representation of the current task status code.
    value: FastCasState,
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
            value: FastCasState::new(status.as_usize() as u64),
        }
    }

    /// Loads the current task status.
    ///
    /// # Returns
    ///
    /// The currently observed task status.
    #[inline]
    pub(crate) fn load(&self) -> TaskStatus {
        TaskStatus::from_usize(self.value.load() as usize)
    }

    /// Attempts to move a pending task into running state.
    ///
    /// # Returns
    ///
    /// `true` if the state changed from pending to running.
    #[inline]
    pub(crate) fn try_start(&self) -> bool {
        self.try_transition(TaskStatusEvent::Start)
    }

    /// Attempts to cancel the task while it is pending.
    ///
    /// # Returns
    ///
    /// `true` if the state changed from pending to cancelled.
    #[inline]
    pub(crate) fn try_cancel_pending(&self) -> bool {
        self.try_transition(TaskStatusEvent::CancelPending)
    }

    /// Attempts to complete a running task with a normal terminal status.
    ///
    /// # Parameters
    ///
    /// * `status` - Success, failure, or panic status represented by the task
    ///   result.
    ///
    /// # Returns
    ///
    /// `true` if the state changed from running to `status`; `false` for
    /// cancellation or dropped statuses, which are handled by explicit APIs.
    #[inline]
    pub(crate) fn try_complete(&self, status: TaskStatus) -> bool {
        let Some(event) = TaskStatusEvent::from_completion_status(status)
        else {
            return false;
        };
        self.try_transition(event)
    }

    /// Attempts to mark a pending or running task as dropped.
    ///
    /// # Returns
    ///
    /// `true` if the state changed from pending or running to dropped.
    #[inline]
    pub(crate) fn try_drop_unfinished(&self) -> bool {
        self.try_transition(TaskStatusEvent::DropUnfinished)
    }

    /// Applies one event through the shared task status machine.
    ///
    /// # Parameters
    ///
    /// * `event` - Event to apply to the current task status.
    ///
    /// # Returns
    ///
    /// `true` if the configured transition exists and the CAS update succeeds.
    #[inline]
    fn try_transition(&self, event: TaskStatusEvent) -> bool {
        TASK_STATUS_MACHINE.try_trigger(&self.value, event.as_usize())
    }
}
