/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for [`DirectExecutor`](qubit_executor::executor::DirectExecutor).

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
use qubit_function::{
    BoxCallable,
    BoxRunnable,
    Callable,
    Runnable,
};

#[test]
fn test_direct_executor_execute_runs_inline() {
    let executor = DirectExecutor;
    let value = Arc::new(AtomicUsize::new(0));
    let value_for_task = Arc::clone(&value);

    let result = executor.execute(move || {
        value_for_task.fetch_add(1, Ordering::AcqRel);
        Ok::<(), io::Error>(())
    });

    result.expect("direct executor should return runnable success");
    assert_eq!(value.load(Ordering::Acquire), 1);
}

#[test]
fn test_direct_executor_call_returns_value() {
    let executor = DirectExecutor;

    let value = executor
        .call(|| Ok::<i32, io::Error>(42))
        .expect("direct executor should return callable value");

    assert_eq!(value, 42);
}

#[test]
fn test_qubit_function_task_types_remain_compatible() {
    let mut runnable: BoxRunnable<io::Error> = Runnable::into_box(|| Ok::<(), io::Error>(()));
    runnable.run().expect("boxed runnable should run");

    let mut callable: BoxCallable<i32, io::Error> = Callable::into_box(|| Ok::<i32, io::Error>(42));
    assert_eq!(
        callable
            .call()
            .expect("boxed callable should return a value"),
        42,
    );
}
