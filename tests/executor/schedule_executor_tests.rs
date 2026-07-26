// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`ScheduleExecutor`](qubit_executor::executor::ScheduleExecutor).

use std::{
    io,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use qubit_executor::executor::{Executor, ScheduleExecutor};
use qubit_executor::hook::NoopTaskHook;
use qubit_executor::service::SubmissionError;

#[test]
fn test_schedule_executor_runs_task_at_instant() {
    let instant = Instant::now() + Duration::from_millis(80);
    let executor = ScheduleExecutor::at(instant);
    let (started_tx, started_rx) = mpsc::channel();

    assert_eq!(executor.instant(), instant);
    let handle = executor
        .execute(move || {
            started_tx
                .send(Instant::now())
                .expect("test should receive start time");
            Ok::<(), io::Error>(())
        })
        .expect("worker thread should spawn");

    assert!(
        started_rx.recv_timeout(Duration::from_millis(30)).is_err(),
        "task should not start before the scheduled instant",
    );
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
    let executor = ScheduleExecutor::at(Instant::now() - Duration::from_millis(1))
        .with_hook(Arc::new(NoopTaskHook));

    let handle = executor
        .call(|| Ok::<usize, io::Error>(42))
        .expect("worker thread should spawn");

    assert_eq!(handle.get().expect("callable should complete"), 42);
}

#[test]
fn test_schedule_executor_with_hook_waits_until_future_instant() {
    let instant = Instant::now() + Duration::from_millis(20);
    let executor = ScheduleExecutor::at(instant).with_hook(Arc::new(NoopTaskHook));

    let handle = executor
        .call(|| Ok::<usize, io::Error>(42))
        .expect("worker thread should spawn");

    assert_eq!(handle.get().expect("callable should complete"), 42);
}

#[test]
fn test_schedule_executor_reports_worker_spawn_failure() {
    let executor = ScheduleExecutor::at(Instant::now())
        .with_hook(Arc::new(NoopTaskHook))
        .with_stack_size(usize::MAX);

    let result = executor.call(|| Ok::<usize, io::Error>(42));

    assert!(matches!(
        result,
        Err(SubmissionError::WorkerSpawnFailed { .. }),
    ));
}

#[test]
fn test_schedule_executor_reports_worker_spawn_failure_without_hook() {
    let executor = ScheduleExecutor::at(Instant::now()).with_stack_size(usize::MAX);

    let result = executor.call(|| Ok::<usize, io::Error>(42));

    assert!(matches!(
        result,
        Err(SubmissionError::WorkerSpawnFailed { .. }),
    ));
}
