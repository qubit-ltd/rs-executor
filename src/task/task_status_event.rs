// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow inline-tests

use super::task_status::TaskStatus;

/// Event codes accepted by the task status state machine (`#[repr(usize)]`
/// discriminants `0..8`).
#[repr(usize)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum TaskStatusEvent {
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
    /// Returns the fast state-machine event code.
    ///
    /// # Returns
    ///
    /// A stable `u64` code accepted by
    /// [`qubit_state_machine::FastStateMachine`].
    #[inline]
    pub(super) const fn as_u64(self) -> u64 {
        self as u64
    }

    /// Returns the completion event matching a normal running-task terminal
    /// status.
    ///
    /// # Parameters
    ///
    /// * `status` - Terminal status represented by a normal task result.
    ///
    /// # Returns
    ///
    /// `Some(event)` for success, failure, and panic statuses; `None` for
    /// non-terminal states and terminal states owned by explicit cancel/drop
    /// APIs.
    pub(super) fn from_completion_status(status: TaskStatus) -> Option<Self> {
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

#[cfg(test)]
mod tests {
    use super::super::task_status_machine::TASK_STATUS_EVENT_COUNT;
    use super::TaskStatusEvent;

    #[test]
    fn task_status_event_as_u64_matches_stable_discriminants() {
        assert_eq!(TaskStatusEvent::Start.as_u64(), 0);
        assert_eq!(TaskStatusEvent::CancelPending.as_u64(), 1);
        assert_eq!(TaskStatusEvent::CompleteSucceeded.as_u64(), 2);
        assert_eq!(TaskStatusEvent::CompleteFailed.as_u64(), 3);
        assert_eq!(TaskStatusEvent::CompletePanicked.as_u64(), 4);
        assert_eq!(TaskStatusEvent::DropUnfinished.as_u64(), 5);
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
            assert_eq!(event.as_u64(), i as u64, "event index {i}");
        }
    }

    #[test]
    fn task_status_event_count_matches_variants() {
        assert_eq!(
            TaskStatusEvent::DropUnfinished as usize + 1,
            TASK_STATUS_EVENT_COUNT,
            "last event discriminant + 1 must equal TASK_STATUS_EVENT_COUNT"
        );
    }
}
