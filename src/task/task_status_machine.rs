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
use qubit_state_machine::TypedFastStateMachine;

use super::task_status::TaskStatus;
use super::task_status_event::TaskStatusEvent;

/// Shared task status machine used by all task handles.
pub(super) static TASK_STATUS_MACHINE: LazyLock<TypedFastStateMachine<TaskStatus, TaskStatusEvent>> =
    LazyLock::new(build_task_status_machine);

/// Builds the explicit task status transition table.
///
/// # Returns
///
/// A validated fast state machine with task lifecycle transitions.
pub(super) fn build_task_status_machine() -> TypedFastStateMachine<TaskStatus, TaskStatusEvent> {
    let pending = TaskStatus::Pending;
    let running = TaskStatus::Running;
    let succeeded = TaskStatus::Succeeded;
    let failed = TaskStatus::Failed;
    let panicked = TaskStatus::Panicked;
    let cancelled = TaskStatus::Cancelled;
    let dropped = TaskStatus::Dropped;

    TypedFastStateMachine::builder()
        .initial_state(pending)
        .terminal_states(&[succeeded, failed, panicked, cancelled, dropped])
        .cas_policy(FastCasPolicy::spin(16))
        .transition(pending, TaskStatusEvent::Start, running)
        .transition(pending, TaskStatusEvent::CancelPending, cancelled)
        .transition(running, TaskStatusEvent::CompleteSucceeded, succeeded)
        .transition(running, TaskStatusEvent::CompleteFailed, failed)
        .transition(running, TaskStatusEvent::CompletePanicked, panicked)
        .transition(pending, TaskStatusEvent::DropUnfinished, dropped)
        .transition(running, TaskStatusEvent::DropUnfinished, dropped)
        .build()
        .expect("task status state machine must be valid")
}

#[cfg(test)]
mod tests {
    use qubit_state_machine::DenseCode;

    use super::super::task_status::TaskStatus;
    use super::super::task_status_event::TaskStatusEvent;
    use super::build_task_status_machine;

    #[test]
    fn test_task_status_machine_has_exactly_the_lifecycle_edges() {
        let machine = build_task_status_machine();
        assert_eq!(machine.state_count(), 7);
        assert_eq!(machine.event_count(), 6);
        assert_eq!(machine.transition_count(), 7);
        for &state in TaskStatus::VALUES {
            for &event in TaskStatusEvent::VALUES {
                let expected = match (state, event) {
                    (TaskStatus::Pending, TaskStatusEvent::Start) => Some(TaskStatus::Running),
                    (TaskStatus::Pending, TaskStatusEvent::CancelPending) => Some(TaskStatus::Cancelled),
                    (TaskStatus::Running, TaskStatusEvent::CompleteSucceeded) => Some(TaskStatus::Succeeded),
                    (TaskStatus::Running, TaskStatusEvent::CompleteFailed) => Some(TaskStatus::Failed),
                    (TaskStatus::Running, TaskStatusEvent::CompletePanicked) => Some(TaskStatus::Panicked),
                    (TaskStatus::Pending | TaskStatus::Running, TaskStatusEvent::DropUnfinished) => {
                        Some(TaskStatus::Dropped)
                    }
                    _ => None,
                };
                assert_eq!(machine.transition_target(state, event), expected);
                if machine.is_terminal_state(state) {
                    assert!(expected.is_none());
                }
            }
        }
    }

    #[test]
    fn test_task_status_machine_uses_created_cells_for_lifecycle_paths() {
        let machine = build_task_status_machine();
        for terminal_event in [
            TaskStatusEvent::CompleteSucceeded,
            TaskStatusEvent::CompleteFailed,
            TaskStatusEvent::CompletePanicked,
            TaskStatusEvent::DropUnfinished,
        ] {
            let state = machine.create_state();
            assert!(machine.try_trigger(&state, TaskStatusEvent::Start));
            assert_eq!(state.load(), TaskStatus::Running);
            assert!(!machine.try_trigger(&state, TaskStatusEvent::CancelPending));
            assert!(machine.try_trigger(&state, terminal_event));
            assert!(machine.is_terminal_state(state.load()));
            assert!(!machine.try_trigger(&state, TaskStatusEvent::Start));
        }
        let state = machine.create_state();
        assert!(machine.try_trigger(&state, TaskStatusEvent::CancelPending));
        assert_eq!(state.load(), TaskStatus::Cancelled);
    }
}
