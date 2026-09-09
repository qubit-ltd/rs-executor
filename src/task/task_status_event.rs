// =============================================================================
// qubit-style: allow source-test-pair
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow inline-tests

use qubit_state_machine::DenseCode;

use super::task_status::TaskStatus;

/// Event codes accepted by the task status state machine (`#[repr(usize)]`
/// discriminants `0..6`).
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
            TaskStatus::Pending | TaskStatus::Running | TaskStatus::Cancelled | TaskStatus::Dropped => None,
        }
    }
}

impl DenseCode for TaskStatusEvent {
    const VALUES: &'static [Self] = &[
        Self::Start,
        Self::CancelPending,
        Self::CompleteSucceeded,
        Self::CompleteFailed,
        Self::CompletePanicked,
        Self::DropUnfinished,
    ];
    /// Returns the stable compact task-event code.
    #[inline]
    fn code(self) -> u64 {
        self as u64
    }
}

#[cfg(test)]
mod tests {
    use qubit_state_machine::DenseCode;

    use super::TaskStatusEvent;

    #[test]
    fn test_task_status_event_codebook_preserves_discriminants() {
        let expected = [
            TaskStatusEvent::Start,
            TaskStatusEvent::CancelPending,
            TaskStatusEvent::CompleteSucceeded,
            TaskStatusEvent::CompleteFailed,
            TaskStatusEvent::CompletePanicked,
            TaskStatusEvent::DropUnfinished,
        ];
        assert_eq!(TaskStatusEvent::VALUES, expected);
        for (index, &event) in TaskStatusEvent::VALUES.iter().enumerate() {
            assert_eq!(event.code(), index as u64);
        }
    }
}
