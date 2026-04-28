/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for [`ThreadPerTaskExecutorService`](qubit_executor::service::ThreadPerTaskExecutorService).

use std::{
    io,
    sync::{
        Arc,
        atomic::{
            AtomicBool,
            Ordering,
        },
    },
    time::Duration,
};

use qubit_executor::{
    TaskExecutionError,
    service::{
        ExecutorService,
        RejectedExecution,
        ThreadPerTaskExecutorService,
    },
};

/// Creates a current-thread Tokio runtime for driving async termination APIs in sync tests.
fn create_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime for task tests")
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
        .expect("service should accept the shared runnable")
        .get()
        .expect("shared runnable should complete successfully");

    let handle = service
        .submit(|| Err::<(), _>(io::Error::other("task failed")))
        .expect("service should accept the runnable");

    let err = handle
        .get()
        .expect_err("accepted runnable should report task failure through handle");
    assert!(matches!(err, TaskExecutionError::Failed(_)));
}

#[test]
fn test_thread_per_task_executor_service_submit_callable_returns_value() {
    let service = ThreadPerTaskExecutorService::new();

    let handle = service
        .submit_callable(ok_usize_task as fn() -> Result<usize, io::Error>)
        .expect("service should accept the callable");

    assert_eq!(
        handle.get().expect("callable should complete successfully"),
        42,
    );
}

#[test]
fn test_thread_per_task_executor_service_reports_panicked_task() {
    let service = ThreadPerTaskExecutorService::new();

    let handle = service
        .submit(|| -> Result<(), io::Error> { panic!("thread per task service panic") })
        .expect("service should accept panicking task");

    assert!(matches!(handle.get(), Err(TaskExecutionError::Panicked)));
}

#[test]
fn test_thread_per_task_executor_service_shutdown_rejects_new_tasks() {
    let service = ThreadPerTaskExecutorService::new();
    service.shutdown();

    let result = service.submit(ok_unit_task as fn() -> Result<(), io::Error>);

    assert!(matches!(result, Err(RejectedExecution::Shutdown)));
    assert!(service.is_shutdown());
    assert!(service.is_terminated());
}

#[test]
fn test_thread_per_task_executor_service_await_termination_waits_for_tasks() {
    let service = ThreadPerTaskExecutorService::new();
    let completed = Arc::new(AtomicBool::new(false));
    let completed_for_task = Arc::clone(&completed);

    service
        .submit(move || {
            std::thread::sleep(Duration::from_millis(80));
            completed_for_task.store(true, Ordering::Release);
            Ok::<(), io::Error>(())
        })
        .expect("service should accept task");

    service.shutdown();
    create_runtime().block_on(service.await_termination());

    assert!(service.is_terminated());
    assert!(completed.load(Ordering::Acquire));
}

#[test]
fn test_thread_per_task_executor_service_shutdown_now_reports_running_tasks() {
    let service = ThreadPerTaskExecutorService::new();
    let _handle = service
        .submit(|| {
            std::thread::sleep(Duration::from_millis(200));
            Ok::<(), io::Error>(())
        })
        .expect("service should accept task");

    let report = service.shutdown_now();

    assert_eq!(report.queued, 0);
    assert_eq!(report.cancelled, 0);
    assert!(service.is_shutdown());
}
