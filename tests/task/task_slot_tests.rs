/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::io;

use qubit_executor::{
    CancelResult,
    TaskExecutionError,
    task::spi::TaskEndpointPair,
};

/// Test task completion start, completion, and cancellation races through public endpoints.
#[test]
fn test_task_slot_start_run_and_cancel_paths() {
    let (handle, completion) = TaskEndpointPair::<usize, io::Error>::new().into_parts();
    assert!(completion.run(|| Ok(42)));
    assert_eq!(
        handle.get().expect("completed task should return value"),
        42
    );

    let (handle, completion) = TaskEndpointPair::<usize, io::Error>::new().into_tracked_parts();
    assert_eq!(handle.cancel(), CancelResult::Cancelled);
    assert!(!completion.run(|| Ok(42)));
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
}
