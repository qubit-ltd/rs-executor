/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for [`ThreadPerTaskExecutorService`](qubit_executor::service::ThreadPerTaskExecutorService).

use std::{
    io,
    sync::{
        Arc,
        Mutex,
        atomic::Ordering,
    },
    time::Duration,
};

use qubit_atomic::Atomic;
use qubit_executor::{
    ExecutorServiceLifecycle,
    TaskExecutionError,
    TaskStatus,
    hook::{
        NoopTaskHook,
        TaskHook,
        TaskId,
    },
    service::{
        ExecutorService,
        ExecutorServiceBuilderError,
        SubmissionError,
        ThreadPerTaskExecutorService,
    },
};

#[derive(Default)]
struct CountingHook {
    accepted: Atomic<usize>,
    rejected: Atomic<usize>,
    finished: Atomic<usize>,
}

impl TaskHook for CountingHook {
    fn on_accepted(&self, _task_id: TaskId) {
        self.accepted.fetch_add_with_ordering(1, Ordering::AcqRel);
    }

    fn on_rejected(&self, _error: &SubmissionError) {
        self.rejected.fetch_add_with_ordering(1, Ordering::AcqRel);
    }

    fn on_finished(&self, _task_id: TaskId, _status: TaskStatus) {
        self.finished.fetch_add_with_ordering(1, Ordering::AcqRel);
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

fn ok_unit_task() -> Result<(), io::Error> {
    Ok(())
}

fn ok_usize_task() -> Result<usize, io::Error> {
    Ok(42)
}

#[test]
fn test_thread_per_task_executor_service_submit_acceptance_is_not_task_success() {
    let service = ThreadPerTaskExecutorService::new();

    service
        .submit(ok_unit_task as fn() -> Result<(), io::Error>)
        .expect("service should accept the shared runnable");

    let handle = service
        .submit_callable(|| Err::<(), _>(io::Error::other("task failed")))
        .expect("service should accept the runnable");

    let err = handle
        .get()
        .expect_err("accepted runnable should report task failure through handle");
    assert!(matches!(err, TaskExecutionError::Failed(_)));
}

#[test]
fn test_thread_per_task_executor_service_submit_callable_returns_value() {
    let service = ThreadPerTaskExecutorService::builder()
        .hook(Arc::new(NoopTaskHook))
        .build()
        .expect("service should build");

    let handle = service
        .submit_callable(ok_usize_task as fn() -> Result<usize, io::Error>)
        .expect("service should accept the callable");

    assert_eq!(
        handle.get().expect("callable should complete successfully"),
        42,
    );
}

#[test]
fn test_thread_per_task_executor_service_hook_events_are_ordered() {
    let hook = Arc::new(RecordingHook::default());
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook.clone())
        .build()
        .expect("service should build");

    service
        .submit_callable(ok_usize_task as fn() -> Result<usize, io::Error>)
        .expect("service should accept the callable")
        .get()
        .expect("task should succeed");

    assert_eq!(hook.events(), vec!["accepted", "started", "finished"]);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_submit_with_hook_runs_task() {
    let hook = Arc::new(RecordingHook::default());
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook.clone())
        .build()
        .expect("service should build");

    service
        .submit(ok_unit_task as fn() -> Result<(), io::Error>)
        .expect("service should accept runnable");
    service.shutdown();
    service.wait_termination();

    assert_eq!(hook.events(), vec!["accepted", "started", "finished"]);
}

#[test]
fn test_thread_per_task_executor_service_submit_tracked_with_hook_runs_task() {
    let hook = Arc::new(RecordingHook::default());
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook.clone())
        .build()
        .expect("service should build");

    service
        .submit_tracked_callable(ok_usize_task as fn() -> Result<usize, io::Error>)
        .expect("service should accept tracked callable")
        .get()
        .expect("task should succeed");

    assert_eq!(hook.events(), vec!["accepted", "started", "finished"]);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_reports_panicked_task() {
    let service = ThreadPerTaskExecutorService::new();

    let handle = service
        .submit_callable(|| -> Result<(), io::Error> { panic!("thread per task service panic") })
        .expect("service should accept panicking task");

    assert!(matches!(handle.get(), Err(TaskExecutionError::Panicked)));
}

#[test]
fn test_thread_per_task_executor_service_shutdown_rejects_new_tasks() {
    let service = ThreadPerTaskExecutorService::new();
    service.shutdown();

    let result = service.submit(ok_unit_task as fn() -> Result<(), io::Error>);

    assert!(matches!(result, Err(SubmissionError::Shutdown)));
    assert!(service.is_not_running());
    assert!(service.is_terminated());

    let callable_result =
        service.submit_callable(ok_usize_task as fn() -> Result<usize, io::Error>);
    assert!(matches!(callable_result, Err(SubmissionError::Shutdown)));

    let tracked_result =
        service.submit_tracked_callable(ok_usize_task as fn() -> Result<usize, io::Error>);
    assert!(matches!(tracked_result, Err(SubmissionError::Shutdown)));
}

#[test]
fn test_thread_per_task_executor_service_shutdown_rejection_notifies_hook() {
    let hook = Arc::new(CountingHook::default());
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook.clone())
        .build()
        .expect("service should build");
    service.shutdown();

    let result = service.submit(ok_unit_task as fn() -> Result<(), io::Error>);

    assert!(matches!(result, Err(SubmissionError::Shutdown)));
    assert_eq!(hook.accepted.load(), 0);
    assert_eq!(hook.rejected.load(), 1);
    assert_eq!(hook.finished.load(), 0);
}

#[test]
fn test_thread_per_task_executor_service_wait_termination_waits_for_tasks() {
    let service = ThreadPerTaskExecutorService::new();
    let completed = Arc::new(Atomic::new(false));
    let completed_for_task = Arc::clone(&completed);

    service
        .submit(move || {
            std::thread::sleep(Duration::from_millis(80));
            completed_for_task.store(true);
            Ok::<(), io::Error>(())
        })
        .expect("service should accept task");

    service.shutdown();
    assert_eq!(service.lifecycle(), ExecutorServiceLifecycle::ShuttingDown);
    assert!(service.is_shutting_down());
    assert!(service.is_not_running());
    service.wait_termination();

    assert_eq!(service.lifecycle(), ExecutorServiceLifecycle::Terminated);
    assert!(service.is_terminated());
    assert!(completed.load());
}

#[test]
fn test_thread_per_task_executor_service_lifecycle_defaults_to_running() {
    let service = ThreadPerTaskExecutorService::new();

    assert_eq!(service.lifecycle(), ExecutorServiceLifecycle::Running);
    assert!(service.is_running());
    assert!(!service.is_not_running());
    assert!(!service.is_shutting_down());
    assert!(!service.is_stopping());
    assert!(!service.is_terminated());
}

#[test]
fn test_thread_per_task_executor_service_stop_reports_running_tasks() {
    let service = ThreadPerTaskExecutorService::new();
    service
        .submit(|| {
            std::thread::sleep(Duration::from_millis(200));
            Ok::<(), io::Error>(())
        })
        .expect("service should accept task");

    let report = service.stop();

    assert_eq!(report.queued, 0);
    assert_eq!(report.cancelled, 0);
    assert_eq!(service.lifecycle(), ExecutorServiceLifecycle::Stopping);
    assert!(service.is_stopping());
    assert!(service.is_not_running());
}

#[test]
fn test_thread_per_task_executor_service_stop_transitions_to_terminated() {
    let service = ThreadPerTaskExecutorService::new();
    service.stop();

    assert_eq!(service.lifecycle(), ExecutorServiceLifecycle::Terminated);
    assert!(service.is_terminated());
}

#[test]
fn test_thread_per_task_executor_service_submit_callable_reports_worker_spawn_failure() {
    let hook = Arc::new(CountingHook::default());
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook.clone())
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = service.submit_callable(|| Ok::<usize, io::Error>(42));

    assert!(matches!(
        result,
        Err(SubmissionError::WorkerSpawnFailed { .. })
    ));
    assert_eq!(hook.accepted.load(), 0);
    assert_eq!(hook.rejected.load(), 1);
    assert_eq!(hook.finished.load(), 0);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_submit_callable_reports_worker_spawn_failure_without_hook()
{
    let service = ThreadPerTaskExecutorService::builder()
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = service.submit_callable(|| Ok::<usize, io::Error>(42));

    assert!(matches!(
        result,
        Err(SubmissionError::WorkerSpawnFailed { .. })
    ));
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_submit_reports_worker_spawn_failure() {
    let service = ThreadPerTaskExecutorService::builder()
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = service.submit(ok_unit_task as fn() -> Result<(), io::Error>);

    assert!(matches!(
        result,
        Err(SubmissionError::WorkerSpawnFailed { .. })
    ));
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_submit_reports_worker_spawn_failure_with_hook() {
    let hook = Arc::new(CountingHook::default());
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook.clone())
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = service.submit(ok_unit_task as fn() -> Result<(), io::Error>);

    assert!(matches!(
        result,
        Err(SubmissionError::WorkerSpawnFailed { .. })
    ));
    assert_eq!(hook.accepted.load(), 0);
    assert_eq!(hook.rejected.load(), 1);
    assert_eq!(hook.finished.load(), 0);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_submit_tracked_reports_worker_spawn_failure() {
    let service = ThreadPerTaskExecutorService::builder()
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = service.submit_tracked_callable(ok_usize_task as fn() -> Result<usize, io::Error>);

    assert!(matches!(
        result,
        Err(SubmissionError::WorkerSpawnFailed { .. })
    ));
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_submit_tracked_reports_worker_spawn_failure_with_hook() {
    let hook = Arc::new(CountingHook::default());
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook.clone())
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = service.submit_tracked_callable(ok_usize_task as fn() -> Result<usize, io::Error>);

    assert!(matches!(
        result,
        Err(SubmissionError::WorkerSpawnFailed { .. })
    ));
    assert_eq!(hook.accepted.load(), 0);
    assert_eq!(hook.rejected.load(), 1);
    assert_eq!(hook.finished.load(), 0);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_repeated_shutdown_and_stop_are_idempotent() {
    let service = ThreadPerTaskExecutorService::new();

    service.shutdown();
    service.shutdown();
    let report = service.stop();

    assert_eq!(report.running, 0);
    assert!(service.is_terminated());
}

#[test]
fn test_thread_per_task_executor_service_builder_rejects_zero_stack_size() {
    let result = ThreadPerTaskExecutorService::builder()
        .stack_size(0)
        .build();

    assert!(matches!(
        result,
        Err(ExecutorServiceBuilderError::ZeroStackSize)
    ));
}
