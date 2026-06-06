// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for task runner utilities.

use std::io;

use qubit_executor::task::spi::{
    TaskEndpointPair,
    TaskRunner,
};

#[test]
fn test_runner_executes_through_completion() {
    let (handle, completion) =
        TaskEndpointPair::<usize, io::Error>::new().into_parts();

    TaskRunner::new(|| Ok::<usize, io::Error>(42)).run(completion);

    assert_eq!(handle.get().expect("runner should publish result"), 42);
}

#[test]
fn test_runner_executes_through_running_slot() {
    let (handle, completion) =
        TaskEndpointPair::<usize, io::Error>::new().into_parts();
    let running = match completion.try_start() {
        Ok(running) => running,
        Err(_) => panic!("pending completion should start"),
    };

    TaskRunner::new(|| Ok::<usize, io::Error>(42)).run_started(running);

    assert_eq!(handle.get().expect("runner should publish result"), 42);
}

#[test]
fn test_runner_run_detached_discards_result() {
    TaskRunner::new(|| Ok::<usize, io::Error>(42)).run_detached();
}
