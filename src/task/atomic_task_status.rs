/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
// qubit-style: allow inline-tests
use std::sync::LazyLock;

use qubit_cas::{
    FastCasPolicy,
    FastCasState,
};
use qubit_state_machine::FastStateMachine;

use super::task_status::{
    TASK_STATUS_COUNT,
    TaskStatus,
};

/// Number of event codes represented by [`TaskStatusEvent`].
const TASK_STATUS_EVENT_COUNT: usize = 6;

/// Shared task status machine used by all task handles.
static TASK_STATUS_MACHINE: LazyLock<FastStateMachine> = LazyLock::new(build_task_status_machine);

/// Event codes accepted by the task status state machine (`#[repr(usize)]` discriminants `0..8`).
#[repr(usize)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum TaskStatusEvent {
    /// Start running a pending task.
    Start = 0,
    /// Cancel a task before it starts.
    CancelPending = 1,
    /// Complete a running task successfully.
    CompleteSucceeded = 2,
    /// Complete a running task with a user error.
    CompleteFailed = 3,
    /// Complete a running task after panic conversion.
    CompletePanicked = 4,
    /// Drop a pending or running task slot before normal completion.
    DropUnfinished = 5,
}

impl TaskStatusEvent {
    /// Returns the compact event code used by [`FastStateMachine`].
    ///
    /// # Returns
    ///
    /// A stable integer code in `0..TASK_STATUS_EVENT_COUNT`.
    #[inline]
    const fn as_usize(self) -> usize {
        self as usize
    }

    /// Returns the completion event matching a normal running-task terminal status.
    ///
    /// # Parameters
    ///
    /// * `status` - Terminal status represented by a normal task result.
    ///
    /// # Returns
    ///
    /// `Some(event)` for success, failure, and panic statuses; `None` for
    /// non-terminal states and terminal states owned by explicit cancel/drop APIs.
    const fn from_completion_status(status: TaskStatus) -> Option<Self> {
        match status {
            TaskStatus::Succeeded => Some(Self::CompleteSucceeded),
            TaskStatus::Failed => Some(Self::CompleteFailed),
            TaskStatus::Panicked => Some(Self::CompletePanicked),
            TaskStatus::Pending
            | TaskStatus::Running
            | TaskStatus::Cancelled
            | TaskStatus::Dropped => None,
        }
    }
}

/// Builds the explicit task status transition table.
///
/// # Returns
///
/// A validated fast state machine with task lifecycle transitions.
fn build_task_status_machine() -> FastStateMachine {
    let pending = TaskStatus::Pending.as_usize();
    let running = TaskStatus::Running.as_usize();
    let succeeded = TaskStatus::Succeeded.as_usize();
    let failed = TaskStatus::Failed.as_usize();
    let panicked = TaskStatus::Panicked.as_usize();
    let cancelled = TaskStatus::Cancelled.as_usize();
    let dropped = TaskStatus::Dropped.as_usize();

    FastStateMachine::builder()
        .state_count(TASK_STATUS_COUNT)
        .event_count(TASK_STATUS_EVENT_COUNT)
        .initial_state(pending)
        .final_states(&[succeeded, failed, panicked, cancelled, dropped])
        .cas_policy(FastCasPolicy::spin(16))
        .transition(pending, TaskStatusEvent::Start.as_usize(), running)
        .transition(
            pending,
            TaskStatusEvent::CancelPending.as_usize(),
            cancelled,
        )
        .transition(
            running,
            TaskStatusEvent::CompleteSucceeded.as_usize(),
            succeeded,
        )
        .transition(running, TaskStatusEvent::CompleteFailed.as_usize(), failed)
        .transition(
            running,
            TaskStatusEvent::CompletePanicked.as_usize(),
            panicked,
        )
        .transition(pending, TaskStatusEvent::DropUnfinished.as_usize(), dropped)
        .transition(running, TaskStatusEvent::DropUnfinished.as_usize(), dropped)
        .build()
        .expect("task status state machine must be valid")
}

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
            value: FastCasState::new(status.as_usize()),
        }
    }

    /// Loads the current task status.
    ///
    /// # Returns
    ///
    /// The currently observed task status.
    #[inline]
    pub(crate) fn load(&self) -> TaskStatus {
        TaskStatus::from_usize(self.value.load())
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
    /// * `status` - Success, failure, or panic status represented by the task result.
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

#[cfg(test)]
mod task_status_event_encoding_tests {
    use super::TaskStatusEvent;

    #[test]
    fn task_status_event_as_usize_matches_stable_discriminants() {
        assert_eq!(TaskStatusEvent::Start.as_usize(), 0);
        assert_eq!(TaskStatusEvent::CancelPending.as_usize(), 1);
        assert_eq!(TaskStatusEvent::CompleteSucceeded.as_usize(), 2);
        assert_eq!(TaskStatusEvent::CompleteFailed.as_usize(), 3);
        assert_eq!(TaskStatusEvent::CompletePanicked.as_usize(), 4);
        assert_eq!(TaskStatusEvent::DropUnfinished.as_usize(), 5);
    }

    #[test]
    fn task_status_event_codes_are_zero_through_seven_in_declaration_order() {
        let events = [
            TaskStatusEvent::Start,
            TaskStatusEvent::CancelPending,
            TaskStatusEvent::CompleteSucceeded,
            TaskStatusEvent::CompleteFailed,
            TaskStatusEvent::CompletePanicked,
            TaskStatusEvent::DropUnfinished,
        ];
        for (i, event) in events.iter().enumerate() {
            assert_eq!(event.as_usize(), i, "event index {i}");
        }
    }

    #[test]
    fn task_status_event_count_matches_variants() {
        assert_eq!(
            TaskStatusEvent::DropUnfinished as usize + 1,
            super::TASK_STATUS_EVENT_COUNT,
            "last event discriminant + 1 must equal TASK_STATUS_EVENT_COUNT"
        );
    }
}
