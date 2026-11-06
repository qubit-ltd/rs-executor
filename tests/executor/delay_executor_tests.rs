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

struct PanicOnDrop(mpsc::Sender<()>);

impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        let _ = self.0.send(());
        panic!("cancelled callable drop panic");
    }
}

struct FinishedCount {
    count: AtomicUsize,
    signal: mpsc::Sender<()>,
    started: AtomicUsize,
}

impl TaskHook for FinishedCount {
    fn on_started(&self, _task_id: TaskId) {
        self.started.fetch_add(1, Ordering::AcqRel);
    }

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
    crate::internal::process_watchdog::run_test(
        &format!(
            "{}::test_delay_executor_cancel_race_preserves_result_and_callable_effects",
            module_path!().strip_prefix("r#mod::").unwrap_or(module_path!())
        ),
        "delay-cancel-race",
        crate::internal::process_watchdog::DEFAULT_TIMEOUT,
        || {
            let (finished_tx, finished_rx) = mpsc::channel();
            let hook = Arc::new(FinishedCount {
                count: AtomicUsize::new(0),
                signal: finished_tx,
                started: AtomicUsize::new(0),
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
                CancelResult::Unsupported => {
                    panic!("tracked delay task cancellation should be supported")
                }
            }
            finished_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("task should emit its finished event");
            assert_eq!(hook.count.load(Ordering::Acquire), 1);
        },
    )
    .expect("watchdog should observe cancellation test finish");
}

#[test]
fn test_cancelled_callable_drop_panic_preserves_cancel_contract() {
    crate::internal::process_watchdog::run_test(
        &format!(
            "{}::test_cancelled_callable_drop_panic_preserves_cancel_contract",
            module_path!().strip_prefix("r#mod::").unwrap_or(module_path!())
        ),
        "delay-cancel-drop-panic",
        crate::internal::process_watchdog::DEFAULT_TIMEOUT,
        || {
            let (finished_tx, finished_rx) = mpsc::channel();
            let hook = Arc::new(FinishedCount {
                count: AtomicUsize::new(0),
                signal: finished_tx,
                started: AtomicUsize::new(0),
            });
            let executor =
                DelayExecutor::new(Duration::from_secs(3600)).with_hook(Arc::clone(&hook) as Arc<dyn TaskHook>);
            let calls = Arc::new(AtomicUsize::new(0));
            let task_calls = Arc::clone(&calls);
            let (drop_started_tx, drop_started_rx) = mpsc::channel();
            let probe = PanicOnDrop(drop_started_tx);
            let handle = executor
                .call(move || {
                    let _keep = &probe;
                    task_calls.fetch_add(1, Ordering::AcqRel);
                    Ok::<usize, io::Error>(42)
                })
                .expect("worker thread should spawn");

            let cancellation = handle.cancel();
            let result = handle.get();
            drop_started_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("cancelled callable cleanup should begin");
            finished_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("task should emit its finished event");

            assert_eq!(cancellation, CancelResult::Cancelled);
            assert!(matches!(result, Err(TaskExecutionError::Cancelled)));
            assert_eq!(calls.load(Ordering::Acquire), 0);
            assert_eq!(hook.started.load(Ordering::Acquire), 0);
            assert_eq!(hook.count.load(Ordering::Acquire), 1);
        },
    )
    .expect("watchdog should observe cancellation test finish");
}

#[test]
fn test_delay_executor_cancel_releases_long_delayed_callable() {
    crate::internal::process_watchdog::run_test(
        &format!(
            "{}::test_delay_executor_cancel_releases_long_delayed_callable",
            module_path!().strip_prefix("r#mod::").unwrap_or(module_path!())
        ),
        "delay-cancel-long-release",
        crate::internal::process_watchdog::DEFAULT_TIMEOUT,
        || {
            let (finished_tx, finished_rx) = mpsc::channel();
            let hook = Arc::new(FinishedCount {
                count: AtomicUsize::new(0),
                signal: finished_tx,
                started: AtomicUsize::new(0),
            });
            let executor =
                DelayExecutor::new(Duration::from_secs(3600)).with_hook(Arc::clone(&hook) as Arc<dyn TaskHook>);
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
            let finished = finished_rx.recv_timeout(Duration::from_secs(5)).is_ok();
            let executions = calls.load(Ordering::Acquire);
            let started = hook.started.load(Ordering::Acquire);
            let finished_count = hook.count.load(Ordering::Acquire);

            assert_eq!(cancellation, CancelResult::Cancelled);
            assert!(matches!(result, Err(TaskExecutionError::Cancelled)));
            assert_eq!(executions, 0);
            assert!(released, "cancelled callable capture should be released promptly");
            assert!(finished, "task should emit its finished event");
            assert_eq!(started, 0);
            assert_eq!(finished_count, 1);
        },
    )
    .expect("watchdog should observe cancellation test finish");
}

#[test]
fn test_delay_executor_cancel_after_start_preserves_running_task() {
    crate::internal::process_watchdog::run_test(
        &format!(
            "{}::test_delay_executor_cancel_after_start_preserves_running_task",
            module_path!().strip_prefix("r#mod::").unwrap_or(module_path!())
        ),
        "delay-cancel-after-start",
        crate::internal::process_watchdog::DEFAULT_TIMEOUT,
        || {
            let (finished_tx, finished_rx) = mpsc::channel();
            let hook = Arc::new(FinishedCount {
                count: AtomicUsize::new(0),
                signal: finished_tx,
                started: AtomicUsize::new(0),
            });
            let executor = DelayExecutor::new(Duration::ZERO).with_hook(Arc::clone(&hook) as Arc<dyn TaskHook>);
            let calls = Arc::new(AtomicUsize::new(0));
            let task_calls = Arc::clone(&calls);
            let (entered_tx, entered_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let (value_tx, value_rx) = mpsc::channel();
            let handle = executor
                .call(move || {
                    entered_tx.send(()).expect("observer exists");
                    release_rx
                        .recv_timeout(Duration::from_secs(5))
                        .expect("task should be released");
                    task_calls.fetch_add(1, Ordering::AcqRel);
                    Ok::<usize, io::Error>(42)
                })
                .expect("worker should spawn");

            let entered = entered_rx.recv_timeout(Duration::from_secs(5));
            let cancellation = handle.cancel();
            let _ = release_tx.send(());
            // Collect get on an observer thread so this test's waiting remains bounded.
            std::thread::spawn(move || {
                let _ = value_tx.send(handle.get());
            });
            let result = value_rx.recv_timeout(Duration::from_secs(5));
            let finished = finished_rx.recv_timeout(Duration::from_secs(5));

            assert!(entered.is_ok(), "task must enter before cancellation");
            assert_eq!(cancellation, CancelResult::AlreadyRunning);
            assert!(matches!(result, Ok(Ok(42))));
            assert!(finished.is_ok(), "finished event should arrive");
            assert_eq!(calls.load(Ordering::Acquire), 1);
            assert_eq!(hook.started.load(Ordering::Acquire), 1);
            assert_eq!(hook.count.load(Ordering::Acquire), 1);
        },
    )
    .expect("watchdog should observe cancellation test finish");
}

#[test]
fn test_delay_executor_cancel_after_try_get_releases_callable() {
    crate::internal::process_watchdog::run_test(
        &format!(
            "{}::test_delay_executor_cancel_after_try_get_releases_callable",
            module_path!().strip_prefix("r#mod::").unwrap_or(module_path!())
        ),
        "delay-cancel-after-try-get",
        crate::internal::process_watchdog::DEFAULT_TIMEOUT,
        || {
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
        },
    )
    .expect("watchdog should observe cancellation test finish");
}

#[test]
fn test_delay_executor_reports_worker_spawn_failure() {
    let executor = DelayExecutor::new(Duration::ZERO)
        .with_hook(Arc::new(NoopTaskHook))
        .with_stack_size(usize::MAX);

    let result = executor.call(delayed_value_task as fn() -> Result<usize, io::Error>);

    assert!(matches!(result, Err(SubmissionError::WorkerSpawnFailed { .. }),));
}
