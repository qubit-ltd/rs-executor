// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::io;

use qubit_executor::CancelResult;
use qubit_executor::TaskExecutionError;
use qubit_executor::TaskStatus;
use qubit_executor::task::spi::TaskEndpointPair;

/// Test task completion start, completion, and cancellation races through
/// public endpoints.
#[test]
fn test_task_slot_start_run_and_cancel_paths() {
    let (handle, completion) =
        TaskEndpointPair::<usize, io::Error>::new().into_parts();
    assert!(completion.run(|| Ok(42)));
    assert_eq!(
        handle.get().expect("completed task should return value"),
        42
    );

    let (handle, completion) =
        TaskEndpointPair::<usize, io::Error>::new().into_tracked_parts();
    assert_eq!(handle.cancel(), CancelResult::Cancelled);
    assert!(!completion.run(|| Ok(42)));
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
}

/// Test explicitly starting a task slot returns a running slot that completes
/// the task.
#[test]
fn test_task_slot_try_start_returns_running_slot() {
    let (handle, completion) =
        TaskEndpointPair::<usize, io::Error>::new().into_tracked_parts();

    completion.accept();
    let running = match completion.try_start() {
        Ok(running) => running,
        Err(_) => panic!("pending completion should start"),
    };

    assert_eq!(handle.status(), TaskStatus::Running);
    assert_eq!(handle.cancel(), CancelResult::AlreadyRunning);
    assert!(running.run(|| Ok(42)));
    assert_eq!(
        handle
            .get()
            .expect("running slot should publish callable result"),
        42,
    );
}

/// Test explicitly starting a cancelled slot fails without running user code.
#[test]
fn test_task_slot_try_start_returns_slot_when_cancelled() {
    let (handle, completion) =
        TaskEndpointPair::<usize, io::Error>::new().into_tracked_parts();

    completion.accept();
    assert_eq!(handle.cancel(), CancelResult::Cancelled);
    let completion = match completion.try_start() {
        Ok(_) => panic!("cancelled completion should not start"),
        Err(completion) => completion,
    };
    drop(completion);

    assert_eq!(handle.status(), TaskStatus::Cancelled);
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
}

/// Test dropping a started running slot reports an abandoned runner endpoint.
#[test]
fn test_running_task_slot_drop_reports_dropped() {
    let (handle, completion) =
        TaskEndpointPair::<usize, io::Error>::new().into_tracked_parts();

    completion.accept();
    let running = match completion.try_start() {
        Ok(running) => running,
        Err(_) => panic!("pending completion should start"),
    };
    drop(running);

    assert_eq!(handle.status(), TaskStatus::Dropped);
    assert!(matches!(handle.get(), Err(TaskExecutionError::Dropped)));
}
