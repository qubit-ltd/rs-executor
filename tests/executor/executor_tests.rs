// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::io;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use qubit_atomic::Atomic;
use qubit_executor::executor::DirectExecutor;
use qubit_executor::executor::Executor;

/// Test the default runnable execution method on the executor trait.
#[test]
fn test_executor_execute_default_delegates_to_call() {
    let executor = DirectExecutor::new();
    let calls = Arc::new(Atomic::new(0usize));
    let calls_for_task = Arc::clone(&calls);

    executor
        .execute(move || {
            calls_for_task.fetch_add_with_ordering(1, Ordering::AcqRel);
            Ok::<(), io::Error>(())
        })
        .expect("direct executor should accept the runnable")
        .get()
        .expect("direct executor should run the runnable");

    assert_eq!(calls.load(), 1);
}
