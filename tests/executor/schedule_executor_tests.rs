// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`ScheduleExecutor`](qubit_executor::executor::ScheduleExecutor).

use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;

use qubit_executor::CancelResult;
use qubit_executor::TaskExecutionError;
use qubit_executor::executor::Executor;
use qubit_executor::executor::ScheduleExecutor;
use qubit_executor::hook::NoopTaskHook;
use qubit_executor::service::SubmissionError;

struct DropSignal(mpsc::Sender<()>);

impl Drop for DropSignal {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

#[test]
fn test_schedule_executor_runs_task_at_instant() {
    let instant = Instant::now() + Duration::from_millis(80);
    let executor = ScheduleExecutor::at(instant);
    let (started_tx, started_rx) = mpsc::channel();

    assert_eq!(executor.instant(), instant);
    let handle = executor
        .execute(move || {
            started_tx.send(Instant::now()).expect("test should receive start time");
            Ok::<(), io::Error>(())
        })
        .expect("worker thread should spawn");

    let started_at = started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("task should start at scheduled instant");
    handle.get().expect("scheduled task should complete");
    assert!(started_at >= instant);
}

#[test]
fn test_schedule_executor_runs_past_instant_promptly() {
    let executor = ScheduleExecutor::at(Instant::now() - Duration::from_millis(1));

    let handle = executor
        .call(|| Ok::<usize, io::Error>(42))
        .expect("worker thread should spawn");

    assert_eq!(handle.get().expect("callable should complete"), 42);
}

#[test]
fn test_schedule_executor_with_hook_runs_past_instant_promptly() {
    let executor = ScheduleExecutor::at(Instant::now() - Duration::from_millis(1)).with_hook(Arc::new(NoopTaskHook));

    let handle = executor
        .call(|| Ok::<usize, io::Error>(42))
        .expect("worker thread should spawn");

    assert_eq!(handle.get().expect("callable should complete"), 42);
}

#[test]
fn test_schedule_executor_with_hook_waits_until_future_instant() {
    let instant = Instant::now() + Duration::from_millis(20);
    let executor = ScheduleExecutor::at(instant).with_hook(Arc::new(NoopTaskHook));
    let (started_tx, started_rx) = mpsc::channel();

    let handle = executor
        .call(move || {
            started_tx
                .send(Instant::now())
                .expect("test should observe task start");
            Ok::<usize, io::Error>(42)
        })
        .expect("worker thread should spawn");

    let started_at = started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("scheduled task should eventually start");
    assert!(
        started_at >= instant,
        "hook-enabled task must not start before its deadline"
    );
    assert_eq!(handle.get().expect("callable should complete"), 42);
}

#[test]
fn test_schedule_executor_reports_worker_spawn_failure() {
    let executor = ScheduleExecutor::at(Instant::now())
        .with_hook(Arc::new(NoopTaskHook))
        .with_stack_size(usize::MAX);

    let result = executor.call(|| Ok::<usize, io::Error>(42));

    assert!(matches!(result, Err(SubmissionError::WorkerSpawnFailed { .. }),));
}

#[test]
fn test_schedule_executor_reports_worker_spawn_failure_without_hook() {
    let executor = ScheduleExecutor::at(Instant::now()).with_stack_size(usize::MAX);

    let result = executor.call(|| Ok::<usize, io::Error>(42));

    assert!(matches!(result, Err(SubmissionError::WorkerSpawnFailed { .. }),));
}

#[test]
fn test_schedule_executor_cancel_releases_long_scheduled_callable() {
    let executor = ScheduleExecutor::at(Instant::now() + Duration::from_secs(3600));
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_task = Arc::clone(&calls);
    let (dropped_tx, dropped_rx) = mpsc::channel();
    let probe = DropSignal(dropped_tx);
    let handle = executor
        .call(move || {
            let _ = &probe;
            calls_for_task.fetch_add(1, Ordering::AcqRel);
            Ok::<usize, io::Error>(42)
        })
        .expect("worker thread should spawn");

    let cancellation = handle.cancel();
    let result = handle.get();
    let released = dropped_rx.recv_timeout(Duration::from_secs(5)).is_ok();
    let executions = calls.load(Ordering::Acquire);

    assert_eq!(cancellation, CancelResult::Cancelled);
    assert!(matches!(result, Err(TaskExecutionError::Cancelled)));
    assert_eq!(executions, 0);
    assert!(released, "cancelled callable capture should be released promptly");
}

#[test]
fn test_schedule_executor_cancel_after_try_get_releases_callable() {
    let executor = ScheduleExecutor::at(Instant::now() + Duration::from_secs(3600));
    let (dropped_tx, dropped_rx) = mpsc::channel();
    let probe = DropSignal(dropped_tx);
    let handle = executor
        .call(move || {
            let _ = &probe;
            Ok::<usize, io::Error>(42)
        })
        .expect("worker thread should spawn");
    let handle = match handle.try_get() {
        qubit_executor::TryGet::Pending(handle) => handle,
        qubit_executor::TryGet::Ready(_) => panic!("future-scheduled task should remain pending"),
    };

    let cancellation = handle.cancel();
    let result = handle.get();
    let released = dropped_rx.recv_timeout(Duration::from_secs(5)).is_ok();

    assert_eq!(cancellation, CancelResult::Cancelled);
    assert!(matches!(result, Err(TaskExecutionError::Cancelled)));
    assert!(released, "cancelled callable capture should be released promptly");
}
