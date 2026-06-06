// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`SingleThreadScheduledExecutorService`](qubit_executor::SingleThreadScheduledExecutorService).

use std::{
    future::IntoFuture,
    sync::mpsc,
    thread,
    time::{
        Duration,
        Instant,
    },
};

use qubit_executor::{
    CancelResult,
    ExecutorService,
    ExecutorServiceBuilderError,
    ExecutorServiceLifecycle,
    ScheduledExecutorService,
    SingleThreadScheduledExecutorService,
    SubmissionError,
    TaskExecutionError,
    TaskStatus,
    TryGet,
    task::spi::{
        TaskResultHandle,
        TrackedTaskHandle,
    },
};

#[test]
fn test_single_thread_scheduled_executor_service_internal_support_paths() {
    qubit_executor::schedule::testing::verify_scheduled_task_ordering();
    qubit_executor::schedule::testing::verify_completable_scheduled_task_cancellation_paths();
    qubit_executor::schedule::testing::verify_single_thread_scheduled_executor_service_inner_paths();
}

#[test]
fn test_single_thread_scheduled_executor_service_runs_earliest_deadline_first()
{
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-earliest")
            .expect("scheduled service should start");
    let (sent_tx, sent_rx) = mpsc::channel::<&'static str>();

    for _ in 0..8 {
        let sent_tx = sent_tx.clone();
        service
            .schedule(Duration::from_millis(250), move || {
                sent_tx.send("long").expect("long task should send");
                Ok::<(), ()>(())
            })
            .expect("long delay should schedule");
    }
    service
        .schedule(Duration::from_millis(30), move || {
            sent_tx.send("short").expect("short task should send");
            Ok::<(), ()>(())
        })
        .expect("short delay should schedule");

    assert_eq!(
        sent_rx
            .recv_timeout(Duration::from_millis(150))
            .expect("short delay should not wait behind long delays"),
        "short"
    );
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_schedule_callable_returns_result()
 {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-callable")
            .expect("scheduled service should start");

    let handle = service
        .schedule_callable(Duration::from_millis(10), || {
            Ok::<usize, ()>(40 + 2)
        })
        .expect("callable should schedule");

    assert_eq!(handle.get().expect("callable should succeed"), 42);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_schedule_at_runs_runnable() {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-at")
            .expect("scheduled service should start");
    let (sent_tx, sent_rx) = mpsc::channel::<()>();

    let handle = service
        .schedule_at(Instant::now() + Duration::from_millis(10), move || {
            sent_tx.send(()).expect("scheduled task should send");
            Ok::<(), ()>(())
        })
        .expect("runnable should schedule at instant");

    sent_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("scheduled task should run");
    assert!(handle.get().is_ok());
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_submit_callable_runs_immediately()
 {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-submit")
            .expect("scheduled service should start");

    let handle = service
        .submit_callable(|| Ok::<usize, ()>(6 * 7))
        .expect("callable should submit");

    assert_eq!(handle.get().expect("callable should succeed"), 42);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_submit_runs_runnable() {
    let service = SingleThreadScheduledExecutorService::new(
        "test-scheduled-submit-runnable",
    )
    .expect("scheduled service should start");
    let (sent_tx, sent_rx) = mpsc::channel::<()>();

    service
        .submit(move || {
            sent_tx.send(()).expect("submitted task should send");
            Ok::<(), ()>(())
        })
        .expect("runnable should submit");

    sent_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("submitted task should run");
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_submit_tracked_callable_reports_status()
 {
    let service = SingleThreadScheduledExecutorService::new(
        "test-scheduled-submit-tracked",
    )
    .expect("scheduled service should start");

    let handle = service
        .submit_tracked_callable(|| Ok::<usize, ()>(42))
        .expect("tracked callable should submit");

    assert_eq!(handle.get().expect("tracked callable should succeed"), 42);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_counts_queued_and_running_tasks()
 {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-counts")
            .expect("scheduled service should start");
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();

    let handle = service
        .schedule_callable(Duration::ZERO, move || {
            started_tx.send(()).expect("test should observe task start");
            release_rx.recv().expect("test should release task");
            Ok::<usize, ()>(42)
        })
        .expect("task should schedule");

    started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("task should start");
    assert_eq!(service.queued_count(), 0);
    assert_eq!(service.running_count(), 1);

    release_tx
        .send(())
        .expect("task should receive release signal");
    assert_eq!(handle.get().expect("task should complete"), 42);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_handle_observation_paths() {
    let service = SingleThreadScheduledExecutorService::new(
        "test-scheduled-handle-observation",
    )
    .expect("scheduled service should start");

    let handle = service
        .schedule_callable(Duration::from_secs(30), || Ok::<usize, ()>(42))
        .expect("task should schedule");
    assert!(!handle.is_done());
    assert_eq!(handle.status(), TaskStatus::Pending);
    let _task_id = handle.task_id();
    assert!(!TaskResultHandle::is_done(&handle));
    assert_eq!(TrackedTaskHandle::status(&handle), TaskStatus::Pending);

    let handle = match handle.try_get() {
        TryGet::Pending(handle) => handle,
        TryGet::Ready(_) => panic!("future task should still be pending"),
    };

    assert_eq!(TrackedTaskHandle::cancel(&handle), CancelResult::Cancelled);
    assert!(handle.is_done());
    assert!(matches!(
        TaskResultHandle::get(handle),
        Err(TaskExecutionError::Cancelled)
    ));
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_handle_try_get_ready_path() {
    let service = SingleThreadScheduledExecutorService::new(
        "test-scheduled-handle-ready",
    )
    .expect("scheduled service should start");
    let handle = service
        .schedule_callable(Duration::ZERO, || Ok::<usize, ()>(42))
        .expect("task should schedule");

    while !handle.is_done() {
        thread::sleep(Duration::from_millis(5));
    }

    match TaskResultHandle::try_get(handle) {
        TryGet::Ready(result) => {
            assert_eq!(result.expect("task should succeed"), 42)
        }
        TryGet::Pending(_) => panic!("completed task should be ready"),
    }
    service.shutdown();
    service.wait_termination();
}

#[tokio::test]
async fn test_single_thread_scheduled_executor_service_handle_await_returns_result()
 {
    let service = SingleThreadScheduledExecutorService::new(
        "test-scheduled-handle-await",
    )
    .expect("scheduled service should start");
    let handle = service
        .schedule_callable(Duration::ZERO, || Ok::<usize, ()>(42))
        .expect("task should schedule");

    assert_eq!(handle.into_future().await.expect("task should succeed"), 42);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_cancel_skips_pending_task() {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-cancel")
            .expect("scheduled service should start");
    let (sent_tx, sent_rx) = mpsc::channel::<()>();

    let handle = service
        .schedule(Duration::from_millis(120), move || {
            sent_tx.send(()).expect("cancelled task should not send");
            Ok::<(), ()>(())
        })
        .expect("delayed task should schedule");

    assert_eq!(handle.cancel(), CancelResult::Cancelled);
    assert_eq!(handle.cancel(), CancelResult::AlreadyFinished);
    assert!(
        sent_rx.recv_timeout(Duration::from_millis(180)).is_err(),
        "cancelled task should not run"
    );
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
    service.shutdown();
    service.wait_termination();
    assert!(service.is_terminated());
}

#[test]
fn test_single_thread_scheduled_executor_service_stop_cancels_pending_task() {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-stop")
            .expect("scheduled service should start");
    let handle = service
        .schedule(Duration::from_secs(10), || Ok::<(), ()>(()))
        .expect("delayed task should schedule");

    let report = service.stop();

    assert_eq!(report.queued, 1);
    assert_eq!(report.cancelled, 1);
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_rejects_after_shutdown() {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-reject")
            .expect("scheduled service should start");

    service.shutdown();
    assert!(service.is_not_running());

    assert!(matches!(
        service.schedule(Duration::ZERO, || Ok::<(), ()>(())),
        Err(SubmissionError::Shutdown)
    ));
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_reports_shutting_down_with_running_work()
 {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-lifecycle")
            .expect("scheduled service should start");
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    service
        .schedule(Duration::ZERO, move || {
            started_tx
                .send(())
                .expect("test should receive task start signal");
            release_rx.recv().expect("test should release task");
            Ok::<(), ()>(())
        })
        .expect("task should schedule");

    started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("task should start");
    service.shutdown();

    assert_eq!(service.lifecycle(), ExecutorServiceLifecycle::ShuttingDown);
    assert!(service.is_shutting_down());

    release_tx
        .send(())
        .expect("task should receive release signal");
    service.wait_termination();
    assert_eq!(service.lifecycle(), ExecutorServiceLifecycle::Terminated);
}

#[test]
fn test_single_thread_scheduled_executor_service_reports_spawn_failure() {
    let result = SingleThreadScheduledExecutorService::with_stack_size(
        "test-scheduled-spawn-failure",
        Some(usize::MAX),
    );

    assert!(matches!(
        result,
        Err(ExecutorServiceBuilderError::SpawnWorker { .. })
    ));
}
