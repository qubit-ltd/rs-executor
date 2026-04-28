/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for task execution error helpers.

use std::{
    error::Error,
    io,
};

use qubit_executor::TaskExecutionError;

#[test]
fn test_task_execution_error_predicates() {
    let failed = TaskExecutionError::Failed(io::Error::other("failed"));
    let panicked = TaskExecutionError::<io::Error>::Panicked;
    let cancelled = TaskExecutionError::<io::Error>::Cancelled;

    assert!(failed.is_failed());
    assert!(!failed.is_panicked());
    assert!(!failed.is_cancelled());
    assert!(panicked.is_panicked());
    assert!(!panicked.is_failed());
    assert!(!panicked.is_cancelled());
    assert!(cancelled.is_cancelled());
    assert!(!cancelled.is_failed());
    assert!(!cancelled.is_panicked());
}

#[test]
fn test_task_execution_error_display() {
    let failed = TaskExecutionError::Failed(io::Error::other("failed"));
    let panicked = TaskExecutionError::<io::Error>::Panicked;
    let cancelled = TaskExecutionError::<io::Error>::Cancelled;

    assert_eq!(failed.to_string(), "task failed: failed");
    assert_eq!(panicked.to_string(), "task panicked");
    assert_eq!(cancelled.to_string(), "task was cancelled");
}

#[test]
fn test_task_execution_error_implements_error() {
    let error = TaskExecutionError::Failed(io::Error::other("failed"));
    let as_error: &dyn Error = &error;

    assert_eq!(as_error.to_string(), "task failed: failed");
}
