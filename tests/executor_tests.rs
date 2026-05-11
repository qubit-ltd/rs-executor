/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::{
    io,
    sync::{
        Arc,
        atomic::{
            AtomicUsize,
            Ordering,
        },
    },
};

use qubit_executor::executor::{
    DirectExecutor,
    Executor,
};

/// Test the default runnable execution method on the executor trait.
#[test]
fn test_executor_execute_default_delegates_to_call() {
    let executor = DirectExecutor;
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_task = Arc::clone(&calls);

    executor
        .execute(move || {
            calls_for_task.fetch_add(1, Ordering::AcqRel);
            Ok::<(), io::Error>(())
        })
        .expect("direct executor should accept the runnable")
        .get()
        .expect("direct executor should run the runnable");

    assert_eq!(calls.load(Ordering::Acquire), 1);
}
