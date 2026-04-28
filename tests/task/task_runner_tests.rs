/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for task runner utilities.

use std::io;

use qubit_executor::TaskCompletionPair;

#[test]
fn test_run_task_executes_through_completion() {
    let (handle, completion) = TaskCompletionPair::<usize, io::Error>::new().into_parts();

    qubit_executor::task_runner::run_task(|| Ok::<usize, io::Error>(42), completion);

    assert_eq!(handle.get().expect("run_task should publish result"), 42);
}
