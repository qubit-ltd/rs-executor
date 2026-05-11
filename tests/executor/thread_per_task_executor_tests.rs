/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for [`ThreadPerTaskExecutor`](qubit_executor::executor::ThreadPerTaskExecutor).

use std::{
    io,
    sync::{
        Arc,
        Mutex,
        atomic::{
            AtomicUsize,
            Ordering,
        },
    },
};

use qubit_executor::{
    TaskExecutionError,
    TaskStatus,
    executor::{
        Executor,
        ThreadPerTaskExecutor,
    },
    hook::{
        NoopTaskHook,
        TaskHook,
        TaskId,
    },
    service::{
        ExecutorServiceBuilderError,
        SubmissionError,
    },
};

static SHARED_RUNNER_TASK_CALLS: AtomicUsize = AtomicUsize::new(0);

#[derive(Default)]
struct CountingHook {
    accepted: AtomicUsize,
    rejected: AtomicUsize,
    finished: AtomicUsize,
}

impl TaskHook for CountingHook {
    fn on_accepted(&self, _task_id: TaskId) {
        self.accepted.fetch_add(1, Ordering::AcqRel);
    }

    fn on_rejected(&self, _error: &SubmissionError) {
        self.rejected.fetch_add(1, Ordering::AcqRel);
    }

    fn on_finished(&self, _task_id: TaskId, _status: TaskStatus) {
        self.finished.fetch_add(1, Ordering::AcqRel);
    }
}

#[derive(Default)]
struct RecordingHook {
    events: Mutex<Vec<&'static str>>,
}

impl RecordingHook {
    fn events(&self) -> Vec<&'static str> {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .clone()
    }
}

impl TaskHook for RecordingHook {
    fn on_accepted(&self, _task_id: TaskId) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push("accepted");
    }

    fn on_started(&self, _task_id: TaskId) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push("started");
    }

    fn on_finished(&self, _task_id: TaskId, _status: TaskStatus) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push("finished");
    }
}

fn shared_runner_task() -> Result<usize, &'static str> {
    match SHARED_RUNNER_TASK_CALLS.fetch_add(1, Ordering::AcqRel) {
        0 => Ok(42),
        1 => Err("shared failure"),
        _ => panic!("shared panic"),
    }
}

#[test]
fn test_thread_per_task_executor_execute_runs_task() {
    let executor = ThreadPerTaskExecutor::new();

    let handle = executor
        .execute(|| Ok::<(), io::Error>(()))
        .expect("worker thread should spawn");

    handle
        .get()
        .expect("thread-per-task executor should run task successfully");
}

#[test]
fn test_thread_per_task_executor_call_returns_value() {
    let executor = ThreadPerTaskExecutor::new().with_hook(Arc::new(NoopTaskHook));

    let handle = executor
        .call(|| Ok::<usize, io::Error>(42))
        .expect("worker thread should spawn");

    assert_eq!(
        handle
            .get()
            .expect("thread-per-task executor should return callable value"),
        42,
    );
}

#[test]
fn test_thread_per_task_executor_hook_events_are_ordered() {
    let hook = Arc::new(RecordingHook::default());
    let executor = ThreadPerTaskExecutor::new().with_hook(hook.clone());

    executor
        .call(|| Ok::<usize, io::Error>(42))
        .expect("worker thread should spawn")
        .get()
        .expect("task should succeed");

    assert_eq!(hook.events(), vec!["accepted", "started", "finished"]);
}

#[test]
fn test_thread_per_task_executor_shared_callable_covers_runner_outcomes() {
    SHARED_RUNNER_TASK_CALLS.store(0, Ordering::Release);
    let executor = ThreadPerTaskExecutor::new();

    let success = executor
        .call(shared_runner_task as fn() -> Result<usize, &'static str>)
        .expect("worker thread should spawn");
    assert_eq!(
        success
            .get()
            .expect("first shared task call should succeed"),
        42,
    );

    let failure = executor
        .call(shared_runner_task as fn() -> Result<usize, &'static str>)
        .expect("worker thread should spawn");
    assert!(matches!(
        failure.get(),
        Err(TaskExecutionError::Failed("shared failure")),
    ));

    let panicked = executor
        .call(shared_runner_task as fn() -> Result<usize, &'static str>)
        .expect("worker thread should spawn");
    assert!(matches!(panicked.get(), Err(TaskExecutionError::Panicked)));
}

#[test]
fn test_thread_per_task_executor_builder_rejects_zero_stack_size() {
    let result = ThreadPerTaskExecutor::builder().stack_size(0).build();

    assert!(matches!(
        result,
        Err(ExecutorServiceBuilderError::ZeroStackSize)
    ));
}

#[test]
fn test_thread_per_task_executor_builder_reports_worker_spawn_failure() {
    let hook = Arc::new(CountingHook::default());
    let executor = ThreadPerTaskExecutor::builder()
        .hook(hook.clone())
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = executor.call(|| Ok::<usize, io::Error>(42));

    assert!(matches!(
        result,
        Err(SubmissionError::WorkerSpawnFailed { .. })
    ));
    assert_eq!(hook.accepted.load(Ordering::Acquire), 0);
    assert_eq!(hook.rejected.load(Ordering::Acquire), 1);
    assert_eq!(hook.finished.load(Ordering::Acquire), 0);
}

#[test]
fn test_thread_per_task_executor_reports_worker_spawn_failure_without_hook() {
    let executor = ThreadPerTaskExecutor::builder()
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = executor.call(|| Ok::<usize, io::Error>(42));

    assert!(matches!(
        result,
        Err(SubmissionError::WorkerSpawnFailed { .. })
    ));
}
