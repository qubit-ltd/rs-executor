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

use qubit_executor::{
    TaskCompletionPair,
    TaskRunner,
};

#[test]
fn test_runner_executes_through_completion() {
    let (handle, completion) = TaskCompletionPair::<usize, io::Error>::new().into_parts();

    TaskRunner::new(|| Ok::<usize, io::Error>(42)).run(completion);

    assert_eq!(handle.get().expect("runner should publish result"), 42);
}
