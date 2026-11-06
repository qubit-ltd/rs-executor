// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests generic async waiting through executor-service handle types.

use std::future::IntoFuture;
use std::time::Duration;

use qubit_executor::CancelResult;
use qubit_executor::ExecutorService;
use qubit_executor::ScheduledExecutorService;
use qubit_executor::SingleThreadScheduledExecutorService;
use qubit_executor::TaskExecutionError;
use qubit_executor::TaskResult;
use qubit_executor::ThreadPerTaskExecutorService;
use qubit_executor::task::spi::TrackedTaskHandle;

async fn result_of<S>(service: &S) -> TaskResult<usize, ()>
where
    S: ExecutorService,
    S::ResultHandle<usize, ()>: IntoFuture<Output = TaskResult<usize, ()>>,
{
    service
        .submit_callable(|| Ok::<usize, ()>(42))
        .expect("submission succeeds")
        .await
}

async fn tracked_result<H>(handle: H) -> TaskResult<usize, ()>
where
    H: TrackedTaskHandle<usize, ()> + IntoFuture<Output = TaskResult<usize, ()>>,
{
    handle.await
}

#[tokio::test]
async fn test_generic_result_handle_can_be_awaited() {
    let service = ThreadPerTaskExecutorService::new();

    let result = tokio::time::timeout(Duration::from_secs(5), result_of(&service))
        .await
        .expect("result handle should complete within five seconds");
    assert_eq!(result.expect("task succeeds"), 42);

    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}

#[tokio::test]
async fn test_generic_tracked_handle_can_be_awaited() {
    let service = SingleThreadScheduledExecutorService::new("generic-async-tracked").expect("scheduled service starts");
    let handle = service
        .schedule_callable(Duration::ZERO, || Ok::<usize, ()>(42))
        .expect("task schedules");

    let result = tokio::time::timeout(Duration::from_secs(5), tracked_result(handle))
        .await
        .expect("tracked handle should complete within five seconds");
    assert_eq!(result.expect("task succeeds"), 42);

    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}

#[tokio::test]
async fn test_generic_tracked_handle_preserves_task_failure() {
    let service = ThreadPerTaskExecutorService::new();
    let handle = service
        .submit_tracked_callable(|| Err::<usize, ()>(()))
        .expect("submission succeeds");

    let result = tokio::time::timeout(Duration::from_secs(5), tracked_result(handle))
        .await
        .expect("result handle should complete within five seconds");
    assert!(matches!(result, Err(TaskExecutionError::Failed(()))));

    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}

#[tokio::test]
async fn test_generic_tracked_handle_preserves_cancellation() {
    let service = SingleThreadScheduledExecutorService::new("generic-async-cancel").expect("scheduled service starts");
    let handle = service
        .schedule_callable(Duration::from_secs(60), || Ok::<usize, ()>(42))
        .expect("task schedules");
    assert_eq!(handle.cancel(), CancelResult::Cancelled);

    let result = tokio::time::timeout(Duration::from_secs(5), tracked_result(handle))
        .await
        .expect("cancelled handle should complete within five seconds");
    assert!(matches!(result, Err(TaskExecutionError::Cancelled)));

    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}
