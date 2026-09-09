// =============================================================================
// qubit-style: allow source-test-pair
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_state_machine::FastStateMachineError;
use qubit_state_machine::TypedFastState;
use qubit_state_machine::TypedFastStateMachineError;

use super::task_status::TaskStatus;
use super::task_status_event::TaskStatusEvent;
use super::task_status_machine::TASK_STATUS_MACHINE;

/// Atomic state machine for one tracked task status.
pub(crate) struct AtomicTaskStatus {
    /// Compact atomic representation of the current task status code.
    value: TypedFastState<TaskStatus>,
}

impl AtomicTaskStatus {
    /// Creates an atomic task status initialized to the machine's pending
    /// state.
    ///
    /// # Returns
    ///
    /// An independent pending task status cell.
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            value: TASK_STATUS_MACHINE.create_state(),
        }
    }

    /// Loads the current task status.
    ///
    /// # Returns
    ///
    /// The currently observed task status.
    #[inline]
    pub(crate) fn load(&self) -> TaskStatus {
        self.value.load()
    }

    /// Returns whether the current status is terminal according to the shared
    /// machine.
    #[inline]
    pub(crate) fn is_terminal(&self) -> bool {
        TASK_STATUS_MACHINE.is_terminal_state(self.value.load())
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
        let Some(event) = TaskStatusEvent::from_completion_status(status) else {
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
    /// The acyclic task graph permits at most two committed state changes.
    /// Strong CAS and a sixteen-attempt budget cannot exhaust on this graph.
    ///
    /// # Panics
    /// Panics on invalid encoding/state or unexpected CAS exhaustion,
    /// indicating a violation of the private graph invariant rather than a
    /// lost task race.
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
        match TASK_STATUS_MACHINE.trigger(&self.value, event) {
            Ok(_) => true,
            Err(TypedFastStateMachineError::Raw(FastStateMachineError::UnknownTransition { .. })) => false,
            Err(error) => panic!("task status machine invariant violated: {error}"),
        }
    }
}
