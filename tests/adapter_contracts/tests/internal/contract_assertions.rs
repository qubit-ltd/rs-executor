// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::future::Future;
use std::future::IntoFuture;
use std::io;
use std::time::Duration;

use qubit_executor::CancelResult;
use qubit_executor::CancellableTaskHandle;
use qubit_executor::ExecutorService;
use qubit_executor::SubmissionError;
use qubit_executor::TaskExecutionError;
use qubit_executor::TaskResult;

pub(crate) async fn assert_pending_cancellation<H>(task: H)
where
    H: CancellableTaskHandle<usize, io::Error> + IntoFuture<Output = TaskResult<usize, io::Error>>,
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
            .expect("cancelled result should arrive"),
        Err(TaskExecutionError::Cancelled)
    ));
}

pub(crate) async fn assert_success<H>(task: H)
where
    H: IntoFuture<Output = TaskResult<usize, io::Error>>,
{
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), task.into_future())
            .await
            .expect("successful result should arrive")
            .expect("task should succeed"),
        42
    );
}

pub(crate) async fn assert_panic_recovery<S>(service: &S)
where
    S: ExecutorService,
    S::ResultHandle<usize, io::Error>: IntoFuture<Output = TaskResult<usize, io::Error>>,
    S::TrackedHandle<usize, io::Error>: IntoFuture<Output = TaskResult<usize, io::Error>>,
{
    let task = service
        .submit_callable(|| -> Result<usize, io::Error> { panic!("ordinary callable contract panic") })
        .expect("panicking callable should be accepted");
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), task.into_future())
            .await
            .expect("ordinary panic result should arrive"),
        Err(TaskExecutionError::Panicked)
    ));
    let next = service
        .submit_callable(|| Ok::<usize, io::Error>(42))
        .expect("ordinary submission after panic should be accepted");
    assert_success(next).await;

    let task = service
        .submit_tracked_callable(|| -> Result<usize, io::Error> { panic!("tracked callable contract panic") })
        .expect("panicking tracked callable should be accepted");
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), task.into_future())
            .await
            .expect("tracked panic result should arrive"),
        Err(TaskExecutionError::Panicked)
    ));
    let next = service
        .submit_tracked_callable(|| Ok::<usize, io::Error>(42))
        .expect("tracked submission after panic should be accepted");
    assert_success(next).await;
}

pub(crate) async fn assert_failure<H>(task: H)
where
    H: IntoFuture<Output = TaskResult<usize, io::Error>>,
{
    match tokio::time::timeout(Duration::from_secs(5), task.into_future())
        .await
        .expect("failed result should arrive")
    {
        Err(TaskExecutionError::Failed(error)) => {
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput)
        }
        result => panic!("expected a task failure, got {result:?}"),
    }
}

pub(crate) fn assert_shutdown_rejection<S>(service: &S)
where
    S: ExecutorService,
{
    service.shutdown();
    let result = service.submit_callable(|| Ok::<usize, io::Error>(42));
    assert!(matches!(result, Err(SubmissionError::Shutdown)));
    assert!(
        service.wait_termination_timeout(Duration::from_secs(5)),
        "service should terminate"
    );
}

pub(crate) async fn assert_runs_after_future_drop<F, O>(task: impl IntoFuture, release: F, observe: O)
where
    F: FnOnce(),
    O: FnOnce(),
{
    drop(task.into_future());
    release();
    observe();
}

pub(crate) async fn assert_runs_after_polled_future_drop<F, O>(task: impl IntoFuture, release: F, observe: O)
where
    F: FnOnce(),
    O: FnOnce(),
{
    let mut result = Box::pin(task.into_future());
    std::future::poll_fn(|cx| {
        assert!(result.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(result);
    release();
    observe();
}
