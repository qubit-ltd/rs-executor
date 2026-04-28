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

use qubit_executor::TaskHandle;

#[test]
fn test_run_task_executes_through_completion() {
    let (handle, completion) = TaskHandle::<usize, io::Error>::completion_pair();

    qubit_executor::task_runner::run_task(|| Ok::<usize, io::Error>(42), completion);

    assert_eq!(handle.get().expect("run_task should publish result"), 42);
}
