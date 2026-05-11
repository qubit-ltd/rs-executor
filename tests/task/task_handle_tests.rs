/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for task handle and completion behavior.

use std::{
    io,
    sync::{
        Arc,
        mpsc,
    },
    thread,
    time::Duration,
};

use qubit_executor::{
    CancelResult,
    TaskCompletionPair,
    TaskExecutionError,
    TaskResultHandle,
    TaskStatus,
    TrackedTaskHandle,
    TryGet,
    executor::{
        Executor,
        ThreadPerTaskExecutor,
    },
    service::RejectedExecution,
};

#[tokio::test]
async fn test_task_handle_await_returns_value() {
    let executor = ThreadPerTaskExecutor;

    let handle = executor.call(|| Ok::<usize, io::Error>(42));

    assert_eq!(
        handle.await.expect("task handle should await task result"),
        42,
    );
}

#[test]
fn test_task_handle_is_done_reports_completion() {
    let executor = ThreadPerTaskExecutor;

    let handle = executor.call(|| Ok::<usize, io::Error>(42));
    for _ in 0..100 {
        if handle.is_done() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(handle.is_done());
    assert_eq!(handle.get().expect("task should complete successfully"), 42);
}

#[test]
fn test_task_handle_cancel_after_start_returns_false() {
    let executor = ThreadPerTaskExecutor;
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();

    let handle = executor.call(move || {
        started_tx
            .send(())
            .expect("test should receive start signal");
        release_rx
            .recv()
            .map_err(|err| io::Error::other(err.to_string()))?;
        Ok::<usize, io::Error>(42)
    });
    started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("task should start within timeout");

    assert_eq!(handle.cancel(), CancelResult::AlreadyRunning);
    release_tx
        .send(())
        .expect("task should receive release signal");
    assert_eq!(handle.get().expect("task should complete"), 42);
}

#[test]
fn test_task_completion_start_and_complete_publishes_lazy_result() {
    let (handle, completion) = TaskCompletionPair::<usize, io::Error>::new().into_parts();

    assert!(completion.start_and_complete(|| Ok(42)));

    assert_eq!(
        handle.get().expect("lazy completion should publish result"),
        42,
    );
}

#[test]
fn test_task_completion_pair_default_creates_usable_pair() {
    let pair = TaskCompletionPair::<usize, io::Error>::default();
    let (handle, completion) = pair.into_parts();

    completion.complete(Ok(42));

    assert_eq!(
        handle.get().expect("default pair should publish result"),
        42
    );
}

#[test]
fn test_task_completion_start_and_complete_skips_cancelled_task() {
    let (handle, completion) = TaskCompletionPair::<usize, io::Error>::new().into_tracked_parts();

    assert_eq!(handle.cancel(), CancelResult::Cancelled);
    assert!(!completion.start_and_complete(|| {
        panic!("cancelled task must not run");
    }));

    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
}

#[test]
fn test_task_completion_clone_can_complete_task() {
    let (handle, completion) = TaskCompletionPair::<usize, io::Error>::new().into_parts();
    let cloned = completion.clone();

    cloned.complete(Ok(42));

    assert_eq!(handle.get().expect("cloned completion should publish"), 42);
}

#[test]
fn test_task_result_handle_trait_methods_cover_task_handle_paths() {
    let (pending_handle, pending_completion) =
        TaskCompletionPair::<usize, io::Error>::new().into_parts();

    assert!(!TaskResultHandle::is_done(&pending_handle));
    let pending_handle = match TaskResultHandle::try_get(pending_handle) {
        TryGet::Pending(handle) => handle,
        TryGet::Ready(_) => panic!("unfinished task should not be ready"),
    };
    pending_completion.complete(Ok(42));
    assert_eq!(
        TaskResultHandle::get(pending_handle).expect("trait get should read result"),
        42,
    );

    let (disconnected_handle, disconnected_completion) =
        TaskCompletionPair::<usize, io::Error>::new().into_parts();
    drop(disconnected_completion);

    assert!(matches!(
        TaskResultHandle::try_get(disconnected_handle),
        TryGet::Ready(Err(TaskExecutionError::Cancelled)),
    ));
}

#[test]
fn test_tracked_task_trait_methods_cover_status_and_cancellation_paths() {
    let (handle, completion) = TaskCompletionPair::<usize, io::Error>::new().into_tracked_parts();

    assert_eq!(TrackedTaskHandle::status(&handle), TaskStatus::Pending);
    assert_eq!(TrackedTaskHandle::cancel(&handle), CancelResult::Cancelled);
    assert!(TaskResultHandle::is_done(&handle));
    assert!(matches!(
        TaskResultHandle::get(handle),
        Err(TaskExecutionError::Cancelled),
    ));
    assert!(!completion.start_and_complete(|| Ok(42)));
}

#[test]
fn test_tracked_task_try_get_returns_pending_and_ready_results() {
    let (handle, completion) = TaskCompletionPair::<usize, io::Error>::new().into_tracked_parts();

    let handle = match handle.try_get() {
        TryGet::Pending(handle) => handle,
        TryGet::Ready(_) => panic!("unfinished tracked task should be pending"),
    };
    completion.complete(Ok(42));

    assert!(matches!(handle.try_get(), TryGet::Ready(Ok(42))));
}

#[test]
fn test_tracked_task_status_reports_failed_and_panicked_results() {
    let (failed_handle, failed_completion) =
        TaskCompletionPair::<usize, io::Error>::new().into_tracked_parts();
    failed_completion.complete(Err(TaskExecutionError::Failed(io::Error::other("failed"))));
    assert_eq!(failed_handle.status(), TaskStatus::Failed);
    assert!(failed_handle.is_done());

    let (panicked_handle, panicked_completion) =
        TaskCompletionPair::<usize, io::Error>::new().into_tracked_parts();
    panicked_completion.complete(Err(TaskExecutionError::Panicked));
    assert_eq!(panicked_handle.status(), TaskStatus::Panicked);
    assert!(panicked_handle.is_done());
}

#[test]
fn test_tracked_task_cancel_reports_finished_for_completed_task() {
    let (handle, completion) = TaskCompletionPair::<usize, io::Error>::new().into_tracked_parts();
    completion.complete(Ok(42));

    assert_eq!(handle.cancel(), CancelResult::AlreadyFinished);
}

#[test]
fn test_tracked_task_trait_cancel_reports_running_and_finished_tasks() {
    let (running_handle, running_completion) =
        TaskCompletionPair::<usize, io::Error>::new().into_tracked_parts();
    assert!(running_completion.start());
    assert_eq!(
        TrackedTaskHandle::cancel(&running_handle),
        CancelResult::AlreadyRunning,
    );
    running_completion.complete(Ok(42));

    let (finished_handle, finished_completion) =
        TaskCompletionPair::<usize, io::Error>::new().into_tracked_parts();
    finished_completion.complete(Ok(42));
    assert_eq!(
        TrackedTaskHandle::cancel(&finished_handle),
        CancelResult::AlreadyFinished,
    );
}

#[test]
fn test_rejected_execution_worker_spawn_failures_compare_by_variant() {
    let first = RejectedExecution::WorkerSpawnFailed {
        source: Arc::new(io::Error::other("first")),
    };
    let second = RejectedExecution::WorkerSpawnFailed {
        source: Arc::new(io::Error::other("second")),
    };

    assert_eq!(first, second);
    assert_ne!(first, RejectedExecution::Shutdown);
}
