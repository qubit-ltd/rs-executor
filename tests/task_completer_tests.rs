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
    TaskExecutionError,
    task::spi::TaskEndpointPair,
};

/// Test task completion start, completion, and cancellation races through public endpoints.
#[test]
fn test_task_completer_start_complete_and_cancel_paths() {
    let (handle, completion) = TaskEndpointPair::<usize, io::Error>::new().into_parts();
    assert!(completion.start());
    assert!(!completion.cancel());
    completion.complete(Ok(42));
    assert_eq!(
        handle.get().expect("completed task should return value"),
        42
    );

    let (handle, completion) = TaskEndpointPair::<usize, io::Error>::new().into_parts();
    assert!(completion.cancel());
    assert!(!completion.start());
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
}
