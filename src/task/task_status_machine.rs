// =============================================================================
// qubit-style: allow source-test-pair
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow inline-tests

use std::sync::LazyLock;

use qubit_fast_cas::FastCasPolicy;
use qubit_state_machine::FastStateMachine;

use super::task_status::{
    TASK_STATUS_COUNT,
    TaskStatus,
};
use super::task_status_event::TaskStatusEvent;

/// Number of event codes represented by [`TaskStatusEvent`].
pub(super) const TASK_STATUS_EVENT_COUNT: usize = 6;

/// Shared task status machine used by all task handles.
pub(super) static TASK_STATUS_MACHINE: LazyLock<FastStateMachine> =
    LazyLock::new(build_task_status_machine);

/// Builds the explicit task status transition table.
///
/// # Returns
///
/// A validated fast state machine with task lifecycle transitions.
pub(super) fn build_task_status_machine() -> FastStateMachine {
    let pending = TaskStatus::Pending.as_u64();
    let running = TaskStatus::Running.as_u64();
    let succeeded = TaskStatus::Succeeded.as_u64();
    let failed = TaskStatus::Failed.as_u64();
    let panicked = TaskStatus::Panicked.as_u64();
    let cancelled = TaskStatus::Cancelled.as_u64();
    let dropped = TaskStatus::Dropped.as_u64();

    FastStateMachine::builder()
        .state_count(TASK_STATUS_COUNT as u64)
        .event_count(TASK_STATUS_EVENT_COUNT as u64)
        .initial_state(pending)
        .final_states(&[succeeded, failed, panicked, cancelled, dropped])
        .cas_policy(FastCasPolicy::spin(16))
        .transition(pending, TaskStatusEvent::Start.as_u64(), running)
        .transition(pending, TaskStatusEvent::CancelPending.as_u64(), cancelled)
        .transition(
            running,
            TaskStatusEvent::CompleteSucceeded.as_u64(),
            succeeded,
        )
        .transition(running, TaskStatusEvent::CompleteFailed.as_u64(), failed)
        .transition(
            running,
            TaskStatusEvent::CompletePanicked.as_u64(),
            panicked,
        )
        .transition(pending, TaskStatusEvent::DropUnfinished.as_u64(), dropped)
        .transition(running, TaskStatusEvent::DropUnfinished.as_u64(), dropped)
        .build()
        .expect("task status state machine must be valid")
}

#[cfg(test)]
mod tests {
    use qubit_fast_cas::FastCasState;

    use super::{
        super::task_status::TaskStatus,
        super::task_status_event::TaskStatusEvent,
        build_task_status_machine,
    };

    #[test]
    fn build_task_status_machine_allows_expected_lifecycle_transitions() {
        let machine = build_task_status_machine();

        let state = FastCasState::new(TaskStatus::Pending.as_usize() as u64);
        assert_eq!(
            TaskStatus::from_usize(state.load() as usize),
            TaskStatus::Pending
        );
        assert!(machine.try_trigger(&state, TaskStatusEvent::Start.as_u64()));
        assert_eq!(
            TaskStatus::from_usize(state.load() as usize),
            TaskStatus::Running
        );

        let state = FastCasState::new(TaskStatus::Pending.as_usize() as u64);
        assert!(
            machine
                .try_trigger(&state, TaskStatusEvent::CancelPending.as_u64(),)
        );
        assert_eq!(
            TaskStatus::from_usize(state.load() as usize),
            TaskStatus::Cancelled
        );

        let state = FastCasState::new(TaskStatus::Running.as_usize() as u64);
        assert!(
            machine.try_trigger(
                &state,
                TaskStatusEvent::CompleteSucceeded.as_u64(),
            )
        );
        assert_eq!(
            TaskStatus::from_usize(state.load() as usize),
            TaskStatus::Succeeded
        );

        let state = FastCasState::new(TaskStatus::Running.as_usize() as u64);
        assert!(
            machine
                .try_trigger(&state, TaskStatusEvent::DropUnfinished.as_u64())
        );
        assert_eq!(
            TaskStatus::from_usize(state.load() as usize),
            TaskStatus::Dropped
        );
    }

    #[test]
    fn build_task_status_machine_rejects_invalid_transitions() {
        let machine = build_task_status_machine();

        let state = FastCasState::new(TaskStatus::Succeeded.as_usize() as u64);
        assert!(!machine.try_trigger(&state, TaskStatusEvent::Start.as_u64()));
        assert_eq!(
            TaskStatus::from_usize(state.load() as usize),
            TaskStatus::Succeeded
        );

        let state = FastCasState::new(TaskStatus::Running.as_usize() as u64);
        assert!(
            !machine
                .try_trigger(&state, TaskStatusEvent::CancelPending.as_u64(),)
        );
        assert_eq!(
            TaskStatus::from_usize(state.load() as usize),
            TaskStatus::Running
        );
    }
}
