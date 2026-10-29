// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`DirectExecutor`](qubit_executor::executor::DirectExecutor).

use std::io;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;

use qubit_atomic::Atomic;
use qubit_executor::TaskExecutionError;
use qubit_executor::TryGet;
use qubit_executor::executor::DirectExecutor;
use qubit_executor::executor::Executor;
use qubit_function::BoxCallable;
use qubit_function::BoxRunnable;

#[test]
fn test_direct_executor_execute_runs_inline() {
    let executor = DirectExecutor::new();
    let caller_thread_id = thread::current().id();
    let value = Arc::new(Atomic::new(0usize));
    let value_for_task = Arc::clone(&value);

    let handle = executor
        .execute(move || {
            assert_eq!(
                thread::current().id(),
                caller_thread_id,
                "direct execution must stay on the caller thread"
            );
            value_for_task.fetch_add_with_ordering(1, Ordering::AcqRel);
            Ok::<(), io::Error>(())
        })
        .expect("direct executor should accept runnable");

    assert!(handle.is_done(), "direct execution must finish before returning");
    match handle.try_get() {
        TryGet::Ready(Ok(())) => {}
        TryGet::Ready(Err(error)) => panic!("direct runnable failed: {error}"),
        TryGet::Pending(_) => panic!("direct result must be published before returning"),
    }
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
    match failed.get() {
        Err(TaskExecutionError::Failed(error)) => {
            assert_eq!(error.kind(), io::ErrorKind::Other);
            assert_eq!(error.to_string(), "failed");
        }
        other => panic!("expected the original task failure, got {other:?}"),
    }

    let panicked = executor
        .call(|| -> Result<usize, io::Error> { panic!("direct executor panic") })
        .expect("direct executor should accept callable");
    assert!(
        matches!(panicked.get(), Err(TaskExecutionError::Panicked)),
        "callable panic must be reported as Panicked"
    );
}

#[test]
fn test_qubit_function_task_types_remain_compatible() {
    let executor = DirectExecutor::new();
    let value = Arc::new(Atomic::new(0usize));
    let first_value = Arc::clone(&value);
    let second_value = Arc::clone(&value);
    let runnable: BoxRunnable<io::Error> = BoxRunnable::new(move || {
        first_value.fetch_add_with_ordering(1, Ordering::AcqRel);
        Ok::<(), io::Error>(())
    })
    .and_then(move || {
        second_value.fetch_add_with_ordering(1, Ordering::AcqRel);
        Ok::<(), io::Error>(())
    });

    executor
        .execute(runnable)
        .expect("direct executor should accept composed boxed runnable")
        .get()
        .expect("composed boxed runnable should succeed");
    assert_eq!(value.load(), 2);

    let callable: BoxCallable<i32, io::Error> = BoxCallable::new(|| Ok::<i32, io::Error>(20))
        .map(|value| value + 1)
        .and_then(|value| Ok(value * 2));
    assert_eq!(
        executor
            .call(callable)
            .expect("direct executor should accept composed boxed callable")
            .get()
            .expect("composed boxed callable should return a value"),
        42,
    );
}
