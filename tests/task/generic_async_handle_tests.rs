// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests generic async waiting through executor-service handle types.

use std::future::IntoFuture;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use qubit_executor::CancelResult;
use qubit_executor::CancellableTaskHandle;
use qubit_executor::ExecutorService;
use qubit_executor::ScheduledExecutorService;
use qubit_executor::SingleThreadScheduledExecutorService;
use qubit_executor::TaskExecutionError;
use qubit_executor::TaskResult;
use qubit_executor::ThreadPerTaskExecutorService;
use qubit_executor::TryGet;
use qubit_executor::task::spi::TaskResultHandle;
use qubit_executor::task::spi::TrackedTaskHandle;

struct LegacyTrackedHandle(qubit_executor::TrackedTask<usize, ()>);

impl TaskResultHandle<usize, ()> for LegacyTrackedHandle {
    fn is_done(&self) -> bool {
        self.0.is_done()
    }
    fn get(self) -> TaskResult<usize, ()> {
        self.0.get()
    }
    fn try_get(self) -> TryGet<Self, usize, ()> {
        match self.0.try_get() {
            TryGet::Ready(result) => TryGet::Ready(result),
            TryGet::Pending(task) => TryGet::Pending(Self(task)),
        }
    }
}

impl TrackedTaskHandle<usize, ()> for LegacyTrackedHandle {
    fn status(&self) -> qubit_executor::TaskStatus {
        self.0.status()
    }
    fn cancel(&self) -> CancelResult {
        self.0.cancel()
    }
}

impl IntoFuture for LegacyTrackedHandle {
    type Output = TaskResult<usize, ()>;
    type IntoFuture = <qubit_executor::TrackedTask<usize, ()> as IntoFuture>::IntoFuture;
    fn into_future(self) -> Self::IntoFuture {
        self.0.into_future()
    }
}

async fn cancel_after_timeout<H>(task: H)
where
    H: CancellableTaskHandle<usize, ()> + IntoFuture<Output = TaskResult<usize, ()>>,
{
    let cancellation = CancellableTaskHandle::cancellation_handle(&task);
    let mut result = Box::pin(task.into_future());
    assert!(
        tokio::time::timeout(Duration::from_millis(10), result.as_mut())
            .await
            .is_err()
    );
    assert_eq!(cancellation.cancel(), CancelResult::Cancelled);
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), result)
            .await
            .expect("cancelled task should finish"),
        Err(TaskExecutionError::Cancelled)
    ));
}

async fn drop_future_then_cancel<H>(task: H)
where
    H: CancellableTaskHandle<usize, ()> + IntoFuture<Output = TaskResult<usize, ()>>,
{
    let cancellation = CancellableTaskHandle::cancellation_handle(&task);
    drop(task.into_future());
    assert_eq!(cancellation.cancel(), CancelResult::Cancelled);
}

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
async fn test_legacy_tracked_handle_needs_no_cancellation_extension() {
    let service = ThreadPerTaskExecutorService::new();
    let handle = service.submit_tracked_callable(|| Ok::<usize, ()>(42)).unwrap();
    assert_eq!(tracked_result(LegacyTrackedHandle(handle)).await.unwrap(), 42);
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
    let cancellation = CancellableTaskHandle::cancellation_handle(&task);
    let mut result = Box::pin(task.into_future());
    assert!(
        tokio::time::timeout(Duration::from_millis(10), result.as_mut())
            .await
            .is_err()
    );
    assert_eq!(cancellation.cancel(), CancelResult::Cancelled);
    let outcome = tokio::time::timeout(Duration::from_secs(5), result).await.unwrap();
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
    let service = SingleThreadScheduledExecutorService::new("drop-future-with-cancel-endpoint").unwrap();
    let task = service
        .schedule_callable(Duration::from_secs(3600), || Ok::<usize, ()>(42))
        .unwrap();
    drop_future_then_cancel(task).await;
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}

#[tokio::test]
async fn test_generic_cancellation_handle_cancels_after_timeout() {
    let service = SingleThreadScheduledExecutorService::new("generic-cancel-after-timeout").unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let task_calls = Arc::clone(&calls);
    let task = service
        .schedule_callable(Duration::from_secs(3600), move || {
            task_calls.fetch_add(1, Ordering::SeqCst);
            Ok::<usize, ()>(42)
        })
        .unwrap();

    cancel_after_timeout(task).await;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}

#[tokio::test]
async fn test_generic_cancellation_endpoint_drop_does_not_cancel() {
    let service = SingleThreadScheduledExecutorService::new("generic-drop-cancel-endpoint").unwrap();
    let task = service
        .schedule_callable(Duration::ZERO, || Ok::<usize, ()>(42))
        .unwrap();
    drop(CancellableTaskHandle::cancellation_handle(&task));

    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap(),
        42
    );
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}

#[tokio::test]
async fn test_generic_cancellation_handle_reports_finished_task() {
    let service = SingleThreadScheduledExecutorService::new("generic-cancel-finished").unwrap();
    let task = service
        .schedule_callable(Duration::ZERO, || Ok::<usize, ()>(42))
        .unwrap();
    let cancellation = CancellableTaskHandle::cancellation_handle(&task);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), task.into_future())
            .await
            .unwrap()
            .unwrap(),
        42
    );
    assert_eq!(cancellation.cancel(), CancelResult::AlreadyFinished);
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}

#[tokio::test]
async fn test_generic_cancellation_handle_reports_running_task() {
    let service = SingleThreadScheduledExecutorService::new("generic-cancel-running").unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let task = service
        .schedule_callable(Duration::ZERO, move || {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok::<usize, ()>(42)
        })
        .unwrap();
    let cancellation = CancellableTaskHandle::cancellation_handle(&task);
    let result = task.into_future();
    tokio::task::spawn_blocking(move || started_rx.recv_timeout(Duration::from_secs(5)).unwrap())
        .await
        .unwrap();

    assert_eq!(cancellation.cancel(), CancelResult::AlreadyRunning);
    release_tx.send(()).unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), result)
            .await
            .unwrap()
            .unwrap(),
        42
    );
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}
