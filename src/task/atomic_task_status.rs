/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::sync::LazyLock;

use qubit_cas::FastCasState;
use qubit_state_machine::{
    FastCasPolicy,
    FastStateMachine,
};

use super::task_status::TaskStatus;

/// Number of task status codes represented by [`TaskStatus`].
const TASK_STATUS_COUNT: usize = 7;

/// Number of event codes represented by [`TaskStatusEvent`].
const TASK_STATUS_EVENT_COUNT: usize = 8;

/// Shared task status machine used by all task handles.
static TASK_STATUS_MACHINE: LazyLock<FastStateMachine> = LazyLock::new(build_task_status_machine);

/// Event codes accepted by the task status state machine.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum TaskStatusEvent {
    /// Start running a pending task.
    Start,
    /// Cancel a task before it starts.
    CancelPending,
    /// Complete a running task successfully.
    CompleteSucceeded,
    /// Complete a running task with a user error.
    CompleteFailed,
    /// Complete a running task after panic conversion.
    CompletePanicked,
    /// Complete a running task with a cancellation result.
    CompleteCancelled,
    /// Complete a running task with a dropped-result error.
    CompleteDropped,
    /// Drop a pending or running task slot before normal completion.
    DropUnfinished,
}

impl TaskStatusEvent {
    /// Returns the compact event code used by [`FastStateMachine`].
    ///
    /// # Returns
    ///
    /// A stable integer code in `0..TASK_STATUS_EVENT_COUNT`.
    const fn as_usize(self) -> usize {
        match self {
            Self::Start => 0,
            Self::CancelPending => 1,
            Self::CompleteSucceeded => 2,
            Self::CompleteFailed => 3,
            Self::CompletePanicked => 4,
            Self::CompleteCancelled => 5,
            Self::CompleteDropped => 6,
            Self::DropUnfinished => 7,
        }
    }

    /// Returns the completion event matching a terminal task status.
    ///
    /// # Parameters
    ///
    /// * `status` - Terminal status represented by a task result.
    ///
    /// # Returns
    ///
    /// `Some(event)` for terminal statuses and `None` for non-terminal states.
    const fn from_completion_status(status: TaskStatus) -> Option<Self> {
        match status {
            TaskStatus::Succeeded => Some(Self::CompleteSucceeded),
            TaskStatus::Failed => Some(Self::CompleteFailed),
            TaskStatus::Panicked => Some(Self::CompletePanicked),
            TaskStatus::Cancelled => Some(Self::CompleteCancelled),
            TaskStatus::Dropped => Some(Self::CompleteDropped),
            TaskStatus::Pending | TaskStatus::Running => None,
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
        .transition(
            running,
            TaskStatusEvent::CompleteCancelled.as_usize(),
            cancelled,
        )
        .transition(
            running,
            TaskStatusEvent::CompleteDropped.as_usize(),
            dropped,
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

    /// Attempts to complete a running task with a terminal status.
    ///
    /// # Parameters
    ///
    /// * `status` - Terminal status represented by the task result.
    ///
    /// # Returns
    ///
    /// `true` if the state changed from running to `status`.
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
