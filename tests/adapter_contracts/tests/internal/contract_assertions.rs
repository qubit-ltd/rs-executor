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
use std::sync::Arc;
use std::sync::Barrier;
use std::time::Duration;

use qubit_executor::CancelResult;
use qubit_executor::CancellableTaskHandle;
use qubit_executor::ExecutorService;
use qubit_executor::SubmissionError;
use qubit_executor::TaskExecutionError;
use qubit_executor::TaskResult;

pub(crate) async fn assert_running_cancellation<H>(task: H, release: impl FnOnce())
where
    H: CancellableTaskHandle<usize, io::Error> + IntoFuture<Output = TaskResult<usize, io::Error>>,
{
    let endpoint = CancellableTaskHandle::cancellation_handle(&task);
    assert_eq!(endpoint.cancel(), CancelResult::AlreadyRunning);
    release();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), task.into_future())
            .await
            .expect("running task result should arrive")
            .expect("running task should retain its result"),
        42
    );
    assert_eq!(endpoint.cancel(), CancelResult::AlreadyFinished);
}

pub(crate) fn assert_stop_contract<S>(service: S)
where
    S: ExecutorService,
    S::ResultHandle<(), io::Error>: IntoFuture<Output = TaskResult<(), io::Error>>,
    S::TrackedHandle<usize, io::Error>:
        CancellableTaskHandle<usize, io::Error> + IntoFuture<Output = TaskResult<usize, io::Error>>,
{
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;

    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let blocker = service
        .submit_callable(move || {
            started_tx.send(()).expect("blocker observer should exist");
            release_rx
                .recv_timeout(Duration::from_secs(30))
                .expect("blocker must be released");
            Ok::<(), io::Error>(())
        })
        .expect("blocker should be accepted");
    started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("blocker should start");
    struct Release(Option<mpsc::Sender<()>>);
    impl Release {
        fn run(&mut self) {
            if let Some(tx) = self.0.take() {
                let _ = tx.send(());
            }
        }
    }
    impl Drop for Release {
        fn drop(&mut self) {
            self.run();
        }
    }
    let mut release = Release(Some(release_tx));

    let calls = Arc::new(AtomicUsize::new(0));
    let task_calls = Arc::clone(&calls);
    let queued = service
        .submit_tracked_callable(move || {
            task_calls.fetch_add(1, Ordering::AcqRel);
            Ok::<usize, io::Error>(42)
        })
        .expect("queued task should be accepted");
    let endpoint = CancellableTaskHandle::cancellation_handle(&queued);
    let report = service.stop();
    let _ = report; // StopReport fields differ in meaning between providers.
    assert!(matches!(
        service.submit_callable(|| Ok::<(), io::Error>(())),
        Err(SubmissionError::Shutdown)
    ));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime should build");
    assert!(matches!(
        runtime
            .block_on(async { tokio::time::timeout(Duration::from_secs(5), queued.into_future()).await })
            .expect("queued cancellation should arrive"),
        Err(TaskExecutionError::Cancelled)
    ));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert!(
        !service.wait_termination_timeout(Duration::ZERO),
        "running blocker must keep service alive"
    );
    release.run();
    assert_eq!(
        runtime
            .block_on(async { tokio::time::timeout(Duration::from_secs(5), blocker.into_future()).await })
            .expect("blocker result should arrive")
            .expect("blocker should succeed"),
        ()
    );
    assert!(
        service.wait_termination_timeout(Duration::from_secs(5)),
        "service should terminate after blocker release"
    );
    assert_eq!(endpoint.cancel(), CancelResult::AlreadyFinished);
    assert_eq!(calls.load(Ordering::Acquire), 0);
}

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

pub(crate) async fn assert_concurrent_pending_cancellation<H>(task: H)
where
    H: CancellableTaskHandle<usize, io::Error> + IntoFuture<Output = TaskResult<usize, io::Error>>,
{
    let cancellation = CancellableTaskHandle::cancellation_handle(&task);
    let result = task.into_future();
    let gate = Arc::new(Barrier::new(3));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let gate = Arc::clone(&gate);
            let cancellation = cancellation.clone();
            std::thread::spawn(move || {
                gate.wait();
                cancellation.cancel()
            })
        })
        .collect();
    gate.wait();
    let outcomes: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().expect("cancellation thread should finish"))
        .collect();
    assert_eq!(outcomes.iter().filter(|&&r| r == CancelResult::Cancelled).count(), 1);
    assert_eq!(
        outcomes.iter().filter(|&&r| r == CancelResult::AlreadyFinished).count(),
        1
    );
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
