/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for [`DirectExecutor`](qubit_executor::executor::DirectExecutor).

use std::{
    io,
    sync::{
        Arc,
        atomic::Ordering,
    },
};

use qubit_atomic::Atomic;
use qubit_executor::executor::{
    DirectExecutor,
    Executor,
};
use qubit_function::{
    BoxCallable,
    BoxRunnable,
    Callable,
    Runnable,
};

#[test]
fn test_direct_executor_execute_runs_inline() {
    let executor = DirectExecutor::new();
    let value = Arc::new(Atomic::new(0usize));
    let value_for_task = Arc::clone(&value);

    let result = executor.execute(move || {
        value_for_task.fetch_add_with_ordering(1, Ordering::AcqRel);
        Ok::<(), io::Error>(())
    });

    result
        .expect("direct executor should accept runnable")
        .get()
        .expect("direct executor should return runnable success");
    assert_eq!(value.load(), 1);
}

#[test]
fn test_direct_executor_call_returns_value() {
    let executor = DirectExecutor::new();

    let value = executor
        .call(|| Ok::<i32, io::Error>(42))
        .expect("direct executor should accept callable")
        .get()
        .expect("direct executor should return callable value");

    assert_eq!(value, 42);
}

#[test]
fn test_direct_executor_call_converts_task_failure_and_panic() {
    let executor = DirectExecutor::new();

    let failed = executor
        .call(|| Err::<usize, _>(io::Error::other("failed")))
        .expect("direct executor should accept callable");
    assert!(failed.get().is_err());

    let panicked = executor
        .call(|| -> Result<usize, io::Error> { panic!("direct executor panic") })
        .expect("direct executor should accept callable");
    assert!(panicked.get().is_err());
}

#[test]
fn test_qubit_function_task_types_remain_compatible() {
    let mut runnable: BoxRunnable<io::Error> = Runnable::into_box(|| Ok::<(), io::Error>(()));
    runnable.run().expect("boxed runnable should run");

    let mut callable: BoxCallable<i32, io::Error> = Callable::into_box(|| Ok::<i32, io::Error>(42));
    assert_eq!(callable.call().expect("boxed callable should return a value"), 42,);
}
