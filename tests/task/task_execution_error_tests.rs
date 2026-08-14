// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for TaskExecutionError helpers and Display.

use std::error::Error;
use std::io;

use qubit_executor::TaskExecutionError;

#[test]
fn test_task_execution_error_predicates_and_display() {
    let failed = TaskExecutionError::Failed(io::Error::other("boom"));
    assert!(failed.is_failed());
    assert!(!failed.is_panicked());
    assert!(!failed.is_cancelled());
    assert_eq!(format!("{failed}"), "task failed: boom");

    let panicked = TaskExecutionError::<io::Error>::Panicked;
    assert!(panicked.is_panicked());
    assert!(!panicked.is_failed());
    assert!(!panicked.is_cancelled());
    assert!(!panicked.is_dropped());
    assert_eq!(format!("{panicked}"), "task panicked");

    let cancelled = TaskExecutionError::<io::Error>::Cancelled;
    assert!(cancelled.is_cancelled());
    assert!(!cancelled.is_failed());
    assert!(!cancelled.is_panicked());
    assert!(!cancelled.is_dropped());
    assert_eq!(format!("{cancelled}"), "task was cancelled");

    let dropped = TaskExecutionError::<io::Error>::Dropped;
    assert!(dropped.is_dropped());
    assert!(!dropped.is_failed());
    assert!(!dropped.is_panicked());
    assert!(!dropped.is_cancelled());
    assert_eq!(format!("{dropped}"), "task result was dropped");
}

#[test]
fn test_task_execution_error_implements_error() {
    let error = TaskExecutionError::Failed(io::Error::other("failed"));
    let as_error: &dyn Error = &error;

    assert_eq!(as_error.to_string(), "task failed: failed");
}
