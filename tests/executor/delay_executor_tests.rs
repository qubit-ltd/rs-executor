// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`DelayExecutor`](qubit_executor::executor::DelayExecutor).

use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;

use qubit_executor::CancelResult;
use qubit_executor::TaskExecutionError;
use qubit_executor::TaskStatus;
use qubit_executor::executor::DelayExecutor;
use qubit_executor::executor::Executor;
use qubit_executor::hook::NoopTaskHook;
use qubit_executor::hook::TaskHook;
use qubit_executor::hook::TaskId;
use qubit_executor::service::SubmissionError;
use qubit_executor::task::spi::TaskResultHandle;

fn delayed_value_task() -> Result<usize, io::Error> {
    Ok(42)
}

struct DropSignal(mpsc::Sender<()>);

impl Drop for DropSignal {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

struct FinishedCount {
    count: AtomicUsize,
    signal: mpsc::Sender<()>,
}

impl TaskHook for FinishedCount {
    fn on_finished(&self, _task_id: TaskId, _status: TaskStatus) {
        self.count.fetch_add(1, Ordering::AcqRel);
        let _ = self.signal.send(());
    }
}

#[test]
fn test_delay_executor_delays_task_start() {
    let executor = DelayExecutor::new(Duration::from_millis(80));
    let (started_tx, started_rx) = mpsc::channel();
    let start = Instant::now();

    assert_eq!(executor.delay(), Duration::from_millis(80));
    let handle = executor
        .execute(move || {
            started_tx.send(Instant::now()).expect("test should receive start time");
            Ok::<(), io::Error>(())
        })
        .expect("worker thread should spawn");

    let started_at = started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("task should start after delay");
    handle.get().expect("delayed task should complete");
    assert!(
        started_at.duration_since(start) >= executor.delay(),
        "task started after {:?}, before configured delay {:?}",
        started_at.duration_since(start),
        executor.delay(),
    );
}

#[test]
fn test_delay_executor_returns_callable_value() {
    let executor = DelayExecutor::new(Duration::ZERO).with_hook(Arc::new(NoopTaskHook));

    let handle = executor
        .call(delayed_value_task as fn() -> Result<usize, io::Error>)
        .expect("worker thread should spawn");

    assert_eq!(handle.get().expect("callable should complete"), 42);
}

#[test]
fn test_delay_executor_cancel_race_preserves_result_and_callable_effects() {
    let (finished_tx, finished_rx) = mpsc::channel();
    let hook = Arc::new(FinishedCount {
        count: AtomicUsize::new(0),
        signal: finished_tx,
    });
    let executor = DelayExecutor::new(Duration::ZERO).with_hook(Arc::clone(&hook) as Arc<dyn TaskHook>);
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
    dropped_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("callable capture should be released");

    match cancellation {
        CancelResult::Cancelled => {
            assert!(matches!(result, Err(TaskExecutionError::Cancelled)));
            assert_eq!(calls.load(Ordering::Acquire), 0);
        }
        CancelResult::AlreadyRunning | CancelResult::AlreadyFinished => {
            assert!(matches!(result, Ok(42)));
            assert_eq!(calls.load(Ordering::Acquire), 1);
        }
        CancelResult::Unsupported => panic!("tracked delay task cancellation should be supported"),
    }
    finished_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("task should emit its finished event");
    assert_eq!(hook.count.load(Ordering::Acquire), 1);
}

#[test]
fn test_delay_executor_cancel_releases_long_delayed_callable() {
    let executor = DelayExecutor::new(Duration::from_secs(3600));
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
fn test_delay_executor_cancel_after_try_get_releases_callable() {
    let executor = DelayExecutor::new(Duration::from_secs(3600));
    let (dropped_tx, dropped_rx) = mpsc::channel();
    let probe = DropSignal(dropped_tx);
    let handle = executor
        .call(move || {
            let _ = &probe;
            Ok::<usize, io::Error>(42)
        })
        .expect("worker thread should spawn");
    let handle = match TaskResultHandle::try_get(handle) {
        qubit_executor::TryGet::Pending(handle) => handle,
        qubit_executor::TryGet::Ready(_) => panic!("long-delay task should remain pending"),
    };

    let cancellation = handle.cancel();
    let result = handle.get();
    let released = dropped_rx.recv_timeout(Duration::from_secs(5)).is_ok();

    assert_eq!(cancellation, CancelResult::Cancelled);
    assert!(matches!(result, Err(TaskExecutionError::Cancelled)));
    assert!(released, "cancelled callable capture should be released promptly");
}

#[test]
fn test_delay_executor_reports_worker_spawn_failure() {
    let executor = DelayExecutor::new(Duration::ZERO)
        .with_hook(Arc::new(NoopTaskHook))
        .with_stack_size(usize::MAX);

    let result = executor.call(delayed_value_task as fn() -> Result<usize, io::Error>);

    assert!(matches!(result, Err(SubmissionError::WorkerSpawnFailed { .. }),));
}
