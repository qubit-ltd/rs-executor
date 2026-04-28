/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for [`ThreadPerTaskExecutor`](qubit_executor::executor::ThreadPerTaskExecutor).

use std::{
    io,
    sync::atomic::{
        AtomicUsize,
        Ordering,
    },
};

use qubit_executor::{
    TaskExecutionError,
    executor::{
        Executor,
        ThreadPerTaskExecutor,
    },
};

static SHARED_RUNNER_TASK_CALLS: AtomicUsize = AtomicUsize::new(0);

fn shared_runner_task() -> Result<usize, &'static str> {
    match SHARED_RUNNER_TASK_CALLS.fetch_add(1, Ordering::AcqRel) {
        0 => Ok(42),
        1 => Err("shared failure"),
        _ => panic!("shared panic"),
    }
}

#[test]
fn test_thread_per_task_executor_execute_runs_task() {
    let executor = ThreadPerTaskExecutor;

    let handle = executor.execute(|| Ok::<(), io::Error>(()));

    handle
        .get()
        .expect("thread-per-task executor should run task successfully");
}

#[test]
fn test_thread_per_task_executor_call_returns_value() {
    let executor = ThreadPerTaskExecutor;

    let handle = executor.call(|| Ok::<usize, io::Error>(42));

    assert_eq!(
        handle
            .get()
            .expect("thread-per-task executor should return callable value"),
        42,
    );
}

#[test]
fn test_thread_per_task_executor_shared_callable_covers_runner_outcomes() {
    SHARED_RUNNER_TASK_CALLS.store(0, Ordering::Release);
    let executor = ThreadPerTaskExecutor;

    let success = executor.call(shared_runner_task as fn() -> Result<usize, &'static str>);
    assert_eq!(
        success
            .get()
            .expect("first shared task call should succeed"),
        42,
    );

    let failure = executor.call(shared_runner_task as fn() -> Result<usize, &'static str>);
    assert!(matches!(
        failure.get(),
        Err(TaskExecutionError::Failed("shared failure")),
    ));

    let panicked = executor.call(shared_runner_task as fn() -> Result<usize, &'static str>);
    assert!(matches!(panicked.get(), Err(TaskExecutionError::Panicked)));
}
