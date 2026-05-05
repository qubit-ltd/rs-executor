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
    TaskCompletionPair,
    TaskExecutionError,
};

/// Test observable handle state transitions before and after terminal completion.
#[test]
fn test_task_handle_state_transitions_are_observable() {
    let (handle, completion) = TaskCompletionPair::<usize, io::Error>::new().into_parts();
    assert!(!handle.is_done());

    assert!(handle.cancel());
    assert!(handle.is_done());
    assert!(!completion.start_and_complete(|| Ok(42)));
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
}
