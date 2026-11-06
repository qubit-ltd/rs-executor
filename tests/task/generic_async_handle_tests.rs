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

#[tokio::test]
async fn test_cancellation_handle_after_async_timeout() {
    let service = SingleThreadScheduledExecutorService::new("cancel-after-timeout").unwrap();
    let task = service
        .schedule_callable(Duration::from_secs(3600), || Ok::<usize, ()>(42))
        .unwrap();
    let cancellation = task.cancellation_handle();
    let mut result = Box::pin(task.into_future());
    assert!(
        tokio::time::timeout(Duration::from_millis(10), result.as_mut())
            .await
            .is_err()
    );
    assert_eq!(cancellation.cancel(), CancelResult::Cancelled);
    let outcome = tokio::time::timeout(Duration::from_secs(5), result)
        .await
        .unwrap();
    assert!(matches!(outcome, Err(TaskExecutionError::Cancelled)));
    let other = service
        .schedule_callable(Duration::ZERO, || Ok::<usize, ()>(7))
        .unwrap();
    assert_eq!(other.await.unwrap(), 7);
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}

#[tokio::test]
async fn test_cancellation_handle_remains_usable_after_future_drop() {
    let service =
        SingleThreadScheduledExecutorService::new("drop-future-with-cancel-endpoint").unwrap();
    let task = service
        .schedule_callable(Duration::from_secs(3600), || Ok::<usize, ()>(42))
        .unwrap();
    let cancellation = task.cancellation_handle();
    drop(task.into_future());

    assert_eq!(cancellation.cancel(), CancelResult::Cancelled);
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}
