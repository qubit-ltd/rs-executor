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

use qubit_executor::task::TaskCompletionPair;

/// Test a completion pair can be created with default and split into usable endpoints.
#[test]
fn test_task_completion_pair_default_splits_to_working_endpoints() {
    let pair = TaskCompletionPair::<usize, io::Error>::default();
    let (handle, completion) = pair.into_parts();

    assert!(!handle.is_done());
    assert!(completion.start_and_complete(|| Ok(42)));
    assert!(handle.is_done());
    assert_eq!(handle.get().expect("completion should publish result"), 42);
}
