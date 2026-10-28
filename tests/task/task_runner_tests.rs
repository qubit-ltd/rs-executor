// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for task runner utilities.

use std::io;
use std::panic;
use std::panic::AssertUnwindSafe;

use qubit_executor::TaskExecutionError;
use qubit_executor::task::spi::TaskEndpointPair;
use qubit_executor::task::spi::TaskRunner;

struct PanicOnDrop;

impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        panic!("drop panic");
    }
}

#[test]
fn test_runner_converts_callable_drop_panic() {
    let probe = PanicOnDrop;
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        TaskRunner::new(move || {
            let _keep = &probe;
            Ok::<usize, io::Error>(42)
        })
        .call::<usize, io::Error>()
    }));

    assert!(matches!(
        result.expect("callable drop panic should not escape"),
        Err(TaskExecutionError::Panicked),
    ));
}

#[test]
fn test_runner_converts_payload_drop_panic() {
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        TaskRunner::new(|| -> Result<usize, io::Error> { panic::panic_any(PanicOnDrop) }).call::<usize, io::Error>()
    }));

    assert!(matches!(
        result.expect("panic payload drop panic should not escape"),
        Err(TaskExecutionError::Panicked),
    ));
}

#[test]
fn test_runner_run_publishes_panicked_when_callable_drop_panics() {
    let probe = PanicOnDrop;
    let (handle, completion) = TaskEndpointPair::<usize, io::Error>::new().into_parts();

    TaskRunner::new(move || {
        let _keep = &probe;
        Ok::<usize, io::Error>(42)
    })
    .run(completion);

    assert!(matches!(handle.get(), Err(TaskExecutionError::Panicked)));
}

#[test]
fn test_runner_run_started_publishes_panicked_when_payload_drop_panics() {
    let (handle, completion) = TaskEndpointPair::<usize, io::Error>::new().into_parts();
    let running = match completion.try_start() {
        Ok(running) => running,
        Err(_) => panic!("pending completion should start"),
    };

    TaskRunner::new(|| -> Result<usize, io::Error> { panic::panic_any(PanicOnDrop) }).run_started(running);

    assert!(matches!(handle.get(), Err(TaskExecutionError::Panicked)));
}

#[test]
fn test_runner_executes_through_completion() {
    let (handle, completion) = TaskEndpointPair::<usize, io::Error>::new().into_parts();

    TaskRunner::new(|| Ok::<usize, io::Error>(42)).run(completion);

    assert_eq!(handle.get().expect("runner should publish result"), 42);
}

#[test]
fn test_runner_executes_through_running_slot() {
    let (handle, completion) = TaskEndpointPair::<usize, io::Error>::new().into_parts();
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
