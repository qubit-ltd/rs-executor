/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::{
    io,
    sync::{
        Arc,
        Mutex,
    },
};

use qubit_executor::{
    CancelResult,
    TaskExecutionError,
    TaskStatus,
    hook::{
        NoopTaskHook,
        TaskHook,
        TaskId,
    },
    task::spi::TaskEndpointPair,
};

#[derive(Default)]
struct RecordingHook {
    events: Mutex<Vec<String>>,
}

impl RecordingHook {
    fn events(&self) -> Vec<String> {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .clone()
    }
}

impl TaskHook for RecordingHook {
    fn on_accepted(&self, task_id: TaskId) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push(format!("accepted:{}", task_id.get()));
    }

    fn on_started(&self, task_id: TaskId) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push(format!("started:{}", task_id.get()));
    }

    fn on_finished(&self, task_id: TaskId, status: TaskStatus) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push(format!("finished:{}:{status:?}", task_id.get()));
    }
}

/// Test a completion pair can be created with default and split into usable endpoints.
#[test]
fn test_task_endpoint_pair_default_splits_to_working_endpoints() {
    let pair = TaskEndpointPair::<usize, io::Error>::default();
    let (handle, completion) = pair.into_parts();

    assert!(!handle.is_done());
    assert!(completion.run(|| Ok(42)));
    assert!(handle.is_done());
    assert_eq!(handle.get().expect("completion should publish result"), 42);
}

#[test]
fn test_task_endpoint_pair_with_hook_splits_to_working_endpoints() {
    let pair = TaskEndpointPair::<usize, io::Error>::with_hook(Arc::new(NoopTaskHook));
    let (handle, completion) = pair.into_parts();

    completion.accept();
    assert!(completion.run(|| Ok(42)));
    assert_eq!(handle.get().expect("completion should publish result"), 42);
}

/// Test cancelling an accepted pending task emits a terminal cancellation once.
#[test]
fn test_task_endpoint_pair_cancel_pending_finishes_accepted_task() {
    let hook = Arc::new(RecordingHook::default());
    let pair = TaskEndpointPair::<usize, io::Error>::with_hook(hook.clone());
    let (handle, completion) = pair.into_tracked_parts();

    completion.accept();
    let task_id = handle.task_id().get();

    assert_eq!(handle.cancel(), CancelResult::Cancelled);
    assert_eq!(handle.status(), TaskStatus::Cancelled);
    drop(completion);

    assert_eq!(handle.status(), TaskStatus::Cancelled);
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
    assert_eq!(
        hook.events(),
        vec![
            format!("accepted:{task_id}"),
            format!("finished:{task_id}:Cancelled"),
        ],
    );
}

/// Test runner-side cancellation reports cancellation instead of dropped completion.
#[test]
fn test_task_endpoint_pair_cancel_unstarted_slot_finishes_accepted_task() {
    let hook = Arc::new(RecordingHook::default());
    let pair = TaskEndpointPair::<usize, io::Error>::with_hook(hook.clone());
    let (handle, completion) = pair.into_tracked_parts();

    completion.accept();
    let task_id = handle.task_id().get();

    assert!(completion.cancel_unstarted());
    assert_eq!(handle.status(), TaskStatus::Cancelled);
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
    assert_eq!(
        hook.events(),
        vec![
            format!("accepted:{task_id}"),
            format!("finished:{task_id}:Cancelled"),
        ],
    );
}

/// Test runner-side cancellation before acceptance does not emit lifecycle hooks.
#[test]
fn test_task_endpoint_pair_cancel_unstarted_before_accept_skips_hooks() {
    let hook = Arc::new(RecordingHook::default());
    let pair = TaskEndpointPair::<usize, io::Error>::with_hook(hook.clone());
    let (handle, completion) = pair.into_tracked_parts();

    assert!(completion.cancel_unstarted());
    assert_eq!(handle.status(), TaskStatus::Cancelled);
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
    assert!(
        hook.events().is_empty(),
        "unaccepted cancellation should not emit lifecycle hooks"
    );
}

/// Test dropping an accepted pending slot publishes `Dropped` without a start event.
#[test]
fn test_task_endpoint_pair_drop_pending_finishes_accepted_task() {
    let hook = Arc::new(RecordingHook::default());
    let pair = TaskEndpointPair::<usize, io::Error>::with_hook(hook.clone());
    let (handle, completion) = pair.into_tracked_parts();

    completion.accept();
    let task_id = handle.task_id().get();
    drop(completion);

    assert_eq!(handle.status(), TaskStatus::Dropped);
    assert!(matches!(handle.get(), Err(TaskExecutionError::Dropped)));
    assert_eq!(
        hook.events(),
        vec![
            format!("accepted:{task_id}"),
            format!("finished:{task_id}:Dropped"),
        ],
    );
}
