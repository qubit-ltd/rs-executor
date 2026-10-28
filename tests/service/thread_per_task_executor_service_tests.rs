// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`ThreadPerTaskExecutorService`](qubit_executor::service::ThreadPerTaskExecutorService).

use std::io;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use qubit_atomic::Atomic;
use qubit_executor::ExecutorServiceLifecycle;
use qubit_executor::TaskExecutionError;
use qubit_executor::TaskStatus;
use qubit_executor::hook::NoopTaskHook;
use qubit_executor::hook::TaskHook;
use qubit_executor::hook::TaskId;
use qubit_executor::service::ExecutorService;
use qubit_executor::service::ExecutorServiceBuilderError;
use qubit_executor::service::SubmissionError;
use qubit_executor::service::ThreadPerTaskExecutorService;

#[derive(Default)]
struct CountingHook {
    accepted: Atomic<usize>,
    rejected: Atomic<usize>,
    finished: Atomic<usize>,
}

impl TaskHook for CountingHook {
    fn on_accepted(&self, _task_id: TaskId) {
        self.accepted.fetch_add_with_ordering(1, Ordering::AcqRel);
    }

    fn on_rejected(&self, _error: &SubmissionError) {
        self.rejected.fetch_add_with_ordering(1, Ordering::AcqRel);
    }

    fn on_finished(&self, _task_id: TaskId, _status: TaskStatus) {
        self.finished.fetch_add_with_ordering(1, Ordering::AcqRel);
    }
}

#[derive(Default)]
struct RecordingHook {
    events: Mutex<Vec<&'static str>>,
    events_changed: Condvar,
}

struct PausedAcceptanceHook {
    entered: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
}

impl TaskHook for PausedAcceptanceHook {
    fn on_accepted(&self, _task_id: TaskId) {
        let _ = self.entered.send(());
        if let Ok(release) = self.release.lock() {
            let _ = release.recv_timeout(Duration::from_secs(5));
        }
    }
}

impl RecordingHook {
    /// Records one lifecycle event and wakes threads waiting for hook progress.
    fn record_event(&self, event: &'static str) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push(event);
        self.events_changed.notify_all();
    }

    /// Waits until at least `expected_count` events have been recorded.
    fn wait_for_event_count(&self, expected_count: usize) -> Vec<&'static str> {
        let events = self
            .events
            .lock()
            .expect("events lock should not be poisoned before waiting for events");
        let (events, _) = self
            .events_changed
            .wait_timeout_while(events, Duration::from_secs(1), |events| events.len() < expected_count)
            .expect("events lock should not be poisoned while waiting");
        events.clone()
    }
}

impl TaskHook for RecordingHook {
    fn on_accepted(&self, _task_id: TaskId) {
        self.record_event("accepted");
    }

    fn on_started(&self, _task_id: TaskId) {
        self.record_event("started");
    }

    fn on_finished(&self, _task_id: TaskId, _status: TaskStatus) {
        self.record_event("finished");
    }
}

fn ok_unit_task() -> Result<(), io::Error> {
    Ok(())
}

fn ok_usize_task() -> Result<usize, io::Error> {
    Ok(42)
}

#[test]
fn test_thread_per_task_executor_service_submit_acceptance_is_not_task_success() {
    let service = ThreadPerTaskExecutorService::new();

    service
        .submit(ok_unit_task as fn() -> Result<(), io::Error>)
        .expect("service should accept the shared runnable");

    let handle = service
        .submit_callable(|| Err::<(), _>(io::Error::other("task failed")))
        .expect("service should accept the runnable");

    let err = handle
        .get()
        .expect_err("accepted runnable should report task failure through handle");
    assert!(matches!(err, TaskExecutionError::Failed(_)));
}

#[test]
fn test_thread_per_task_executor_service_submit_callable_returns_value() {
    let service = ThreadPerTaskExecutorService::builder()
        .hook(Arc::new(NoopTaskHook))
        .build()
        .expect("service should build");

    let handle = service
        .submit_callable(ok_usize_task as fn() -> Result<usize, io::Error>)
        .expect("service should accept the callable");

    assert_eq!(handle.get().expect("callable should complete successfully"), 42,);
}

#[test]
fn test_thread_per_task_executor_service_hook_events_are_ordered() {
    let hook = Arc::new(RecordingHook::default());
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook.clone())
        .build()
        .expect("service should build");

    service
        .submit_callable(ok_usize_task as fn() -> Result<usize, io::Error>)
        .expect("service should accept the callable")
        .get()
        .expect("task should succeed");

    assert_eq!(hook.wait_for_event_count(3), vec!["accepted", "started", "finished"],);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_submit_with_hook_runs_task() {
    let hook = Arc::new(RecordingHook::default());
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook.clone())
        .build()
        .expect("service should build");

    service
        .submit(ok_unit_task as fn() -> Result<(), io::Error>)
        .expect("service should accept runnable");
    service.shutdown();
    service.wait_termination();

    assert_eq!(hook.wait_for_event_count(3), vec!["accepted", "started", "finished"],);
}

#[test]
fn test_thread_per_task_executor_service_submit_tracked_with_hook_runs_task() {
    let hook = Arc::new(RecordingHook::default());
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook.clone())
        .build()
        .expect("service should build");

    service
        .submit_tracked_callable(ok_usize_task as fn() -> Result<usize, io::Error>)
        .expect("service should accept tracked callable")
        .get()
        .expect("task should succeed");

    assert_eq!(hook.wait_for_event_count(3), vec!["accepted", "started", "finished"],);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_reports_panicked_task() {
    let service = ThreadPerTaskExecutorService::new();

    let handle = service
        .submit_callable(|| -> Result<(), io::Error> { panic!("thread per task service panic") })
        .expect("service should accept panicking task");

    assert!(matches!(handle.get(), Err(TaskExecutionError::Panicked)));
}

#[test]
fn test_thread_per_task_executor_service_shutdown_rejects_new_tasks() {
    let service = ThreadPerTaskExecutorService::new();
    service.shutdown();

    let result = service.submit(ok_unit_task as fn() -> Result<(), io::Error>);

    assert!(matches!(result, Err(SubmissionError::Shutdown)));
    assert!(service.is_not_running());
    assert!(service.is_terminated());

    let callable_result = service.submit_callable(ok_usize_task as fn() -> Result<usize, io::Error>);
    assert!(matches!(callable_result, Err(SubmissionError::Shutdown)));

    let tracked_result = service.submit_tracked_callable(ok_usize_task as fn() -> Result<usize, io::Error>);
    assert!(matches!(tracked_result, Err(SubmissionError::Shutdown)));
}

#[test]
fn test_thread_per_task_executor_service_shutdown_rejection_notifies_hook() {
    let hook = Arc::new(CountingHook::default());
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook.clone())
        .build()
        .expect("service should build");
    service.shutdown();

    let result = service.submit(ok_unit_task as fn() -> Result<(), io::Error>);

    assert!(matches!(result, Err(SubmissionError::Shutdown)));
    assert_eq!(hook.accepted.load(), 0);
    assert_eq!(hook.rejected.load(), 1);
    assert_eq!(hook.finished.load(), 0);
}

#[test]
fn test_thread_per_task_executor_service_wait_termination_waits_for_tasks() {
    let service = ThreadPerTaskExecutorService::new();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();

    service
        .submit(move || {
            entered_tx.send(()).expect("signal task start");
            release_rx.recv().expect("receive task release");
            Ok::<(), io::Error>(())
        })
        .expect("service should accept task");

    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("task should start before shutdown");
    service.shutdown();
    let lifecycle = service.lifecycle();
    let shutting_down = service.is_shutting_down();
    let not_running = service.is_not_running();
    let terminated_early = service.wait_termination_timeout(Duration::from_millis(20));
    let lifecycle_before_release = service.lifecycle();
    release_tx.send(()).expect("release running task");
    let terminated_after_release = service.wait_termination_timeout(Duration::from_secs(5));

    assert!(!terminated_early);
    assert_ne!(lifecycle_before_release, ExecutorServiceLifecycle::Terminated);
    assert!(terminated_after_release);
    assert_eq!(lifecycle, ExecutorServiceLifecycle::ShuttingDown);
    assert!(shutting_down);
    assert!(not_running);
    assert_eq!(service.lifecycle(), ExecutorServiceLifecycle::Terminated);
    assert!(service.is_terminated());
}

/// Verifies timed termination waits probe immediately and observe a later
/// shutdown.
#[test]
fn test_thread_per_task_executor_service_wait_termination_timeout_observes_shutdown() {
    let service = ThreadPerTaskExecutorService::new();
    assert!(!service.wait_termination_timeout(Duration::ZERO));

    let shutdown_service = service.clone();
    let shutdown_thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        shutdown_service.shutdown();
    });

    assert!(service.wait_termination_timeout(Duration::from_secs(1)));
    shutdown_thread.join().expect("shutdown thread should not panic");
}

#[test]
fn test_thread_per_task_executor_service_wait_termination_timeout_handles_max_duration() {
    let service = ThreadPerTaskExecutorService::new();
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::MAX));
}

#[test]
fn test_thread_per_task_service_max_timeout_waits_for_shutdown() {
    let service = ThreadPerTaskExecutorService::new();
    let shutdown_service = service.clone();
    let shutdown_thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        shutdown_service.shutdown();
    });

    assert!(service.wait_termination_timeout(Duration::MAX));
    shutdown_thread.join().expect("shutdown thread should not panic");
}

#[test]
fn test_thread_per_task_executor_service_lifecycle_defaults_to_running() {
    let service = ThreadPerTaskExecutorService::new();

    assert_eq!(service.lifecycle(), ExecutorServiceLifecycle::Running);
    assert!(service.is_running());
    assert!(!service.is_not_running());
    assert!(!service.is_shutting_down());
    assert!(!service.is_stopping());
    assert!(!service.is_terminated());
}

#[test]
fn test_thread_per_task_executor_service_stop_reports_running_tasks() {
    let service = ThreadPerTaskExecutorService::new();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    service
        .submit(move || {
            entered_tx.send(()).expect("signal task start");
            release_rx.recv().expect("receive task release");
            Ok::<(), io::Error>(())
        })
        .expect("service should accept task");

    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("task should start before stop");
    let report = service.stop();
    let stopping = service.is_stopping();
    let terminated_early = service.wait_termination_timeout(Duration::from_millis(20));
    let lifecycle_before_release = service.lifecycle();
    release_tx.send(()).expect("release running task");
    let terminated_after_release = service.wait_termination_timeout(Duration::from_secs(5));

    assert!(!terminated_early);
    assert_ne!(lifecycle_before_release, ExecutorServiceLifecycle::Terminated);
    assert!(terminated_after_release);
    assert_eq!(report.queued, 0);
    assert_eq!(report.running, 1);
    assert_eq!(report.cancelled, 0);
    assert_eq!(service.lifecycle(), ExecutorServiceLifecycle::Terminated);
    assert!(stopping);
    assert!(service.is_not_running());
}

#[test]
fn test_thread_per_task_executor_service_stop_transitions_to_terminated() {
    let service = ThreadPerTaskExecutorService::new();
    let _ = service.stop();

    assert_eq!(service.lifecycle(), ExecutorServiceLifecycle::Terminated);
    assert!(service.is_terminated());
}

#[test]
fn test_thread_per_task_executor_service_submit_callable_reports_worker_spawn_failure() {
    let hook = Arc::new(CountingHook::default());
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook.clone())
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = service.submit_callable(|| Ok::<usize, io::Error>(42));

    assert!(matches!(result, Err(SubmissionError::WorkerSpawnFailed { .. })));
    assert_eq!(hook.accepted.load(), 0);
    assert_eq!(hook.rejected.load(), 1);
    assert_eq!(hook.finished.load(), 0);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_submit_callable_reports_worker_spawn_failure_without_hook() {
    let service = ThreadPerTaskExecutorService::builder()
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = service.submit_callable(|| Ok::<usize, io::Error>(42));

    assert!(matches!(result, Err(SubmissionError::WorkerSpawnFailed { .. })));
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_submit_reports_worker_spawn_failure() {
    let service = ThreadPerTaskExecutorService::builder()
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = service.submit(ok_unit_task as fn() -> Result<(), io::Error>);

    assert!(matches!(result, Err(SubmissionError::WorkerSpawnFailed { .. })));
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_submit_reports_worker_spawn_failure_with_hook() {
    let hook = Arc::new(CountingHook::default());
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook.clone())
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = service.submit(ok_unit_task as fn() -> Result<(), io::Error>);

    assert!(matches!(result, Err(SubmissionError::WorkerSpawnFailed { .. })));
    assert_eq!(hook.accepted.load(), 0);
    assert_eq!(hook.rejected.load(), 1);
    assert_eq!(hook.finished.load(), 0);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_submit_tracked_reports_worker_spawn_failure() {
    let service = ThreadPerTaskExecutorService::builder()
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = service.submit_tracked_callable(ok_usize_task as fn() -> Result<usize, io::Error>);

    assert!(matches!(result, Err(SubmissionError::WorkerSpawnFailed { .. })));
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_submit_tracked_reports_worker_spawn_failure_with_hook() {
    let hook = Arc::new(CountingHook::default());
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook.clone())
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = service.submit_tracked_callable(ok_usize_task as fn() -> Result<usize, io::Error>);

    assert!(matches!(result, Err(SubmissionError::WorkerSpawnFailed { .. })));
    assert_eq!(hook.accepted.load(), 0);
    assert_eq!(hook.rejected.load(), 1);
    assert_eq!(hook.finished.load(), 0);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_thread_per_task_executor_service_repeated_shutdown_and_stop_are_idempotent() {
    let service = ThreadPerTaskExecutorService::new();

    service.shutdown();
    service.shutdown();
    let report = service.stop();

    assert_eq!(report.running, 0);
    assert!(service.is_terminated());
}

#[test]
fn test_thread_per_task_executor_service_builder_rejects_zero_stack_size() {
    let result = ThreadPerTaskExecutorService::builder().stack_size(0).build();

    assert!(matches!(result, Err(ExecutorServiceBuilderError::ZeroStackSize)));
}

fn test_service_admission_close_during_acceptance(stop: bool) {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let hook = Arc::new(PausedAcceptanceHook {
        entered: entered_tx,
        release: Mutex::new(release_rx),
    });
    let service = ThreadPerTaskExecutorService::builder()
        .hook(hook)
        .build()
        .expect("service should build");
    let (ran_tx, ran_rx) = mpsc::channel();
    let submit_service = service.clone();
    let submitter = std::thread::spawn(move || {
        submit_service.submit_callable(move || {
            ran_tx.send(()).expect("execution observer exists");
            Ok::<(), io::Error>(())
        })
    });

    let entered = entered_rx.recv_timeout(Duration::from_secs(5));
    let (not_terminated_during_acceptance, rejected_during_close) = if entered.is_ok() {
        if stop {
            let _ = service.stop();
        } else {
            service.shutdown();
        }
        (
            !service.wait_termination_timeout(Duration::from_millis(20)),
            matches!(
                service.submit_callable(|| Ok::<(), io::Error>(())),
                Err(SubmissionError::Shutdown)
            ),
        )
    } else {
        (false, false)
    };

    let _ = release_tx.send(());
    let submitted = submitter.join().expect("submitter should return");
    if entered.is_err() {
        let _ = service.stop();
    }
    let accepted_task = submitted.expect("in-flight submission should be accepted");
    accepted_task.get().expect("accepted task should finish");
    let task_ran = ran_rx.recv_timeout(Duration::from_secs(5)).is_ok();
    let terminated = service.wait_termination_timeout(Duration::from_secs(5));

    assert!(entered.is_ok(), "acceptance hook should be reached");
    assert!(not_terminated_during_acceptance);
    assert!(rejected_during_close);
    assert!(task_ran, "stop does not interrupt an accepted OS thread");
    assert!(terminated);
}

#[test]
fn test_service_admission_shutdown_during_acceptance() {
    test_service_admission_close_during_acceptance(false);
}

#[test]
fn test_service_admission_stop_during_acceptance() {
    test_service_admission_close_during_acceptance(true);
}

fn test_service_admission_submitters_race_close(stop: bool) {
    const SUBMITTERS: usize = 4;
    let service = ThreadPerTaskExecutorService::new();
    let barrier = Arc::new(std::sync::Barrier::new(SUBMITTERS + 1));
    let (executed_tx, executed_rx) = mpsc::channel();
    let (results_tx, results_rx) = mpsc::channel();
    let mut submitters = Vec::new();

    for id in 0..SUBMITTERS {
        let service = service.clone();
        let barrier = Arc::clone(&barrier);
        let executed_tx = executed_tx.clone();
        let results_tx = results_tx.clone();
        submitters.push(std::thread::spawn(move || {
            barrier.wait();
            let result = service.submit_callable(move || {
                executed_tx.send(id).expect("execution observer exists");
                Ok::<usize, io::Error>(id)
            });
            results_tx.send((id, result)).expect("result observer exists");
        }));
    }
    drop(executed_tx);
    drop(results_tx);

    barrier.wait();
    if stop {
        let _ = service.stop();
    } else {
        service.shutdown();
    }

    let mut accepted_ids = Vec::new();
    let mut handles = Vec::new();
    let mut rejected = 0;
    for _ in 0..SUBMITTERS {
        let (id, result) = results_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("each submitter should return");
        match result {
            Ok(handle) => {
                accepted_ids.push(id);
                handles.push((id, handle));
            }
            Err(SubmissionError::Shutdown) => rejected += 1,
            Err(error) => panic!("unexpected submission error: {error:?}"),
        }
    }
    for submitter in submitters {
        submitter.join().expect("submitter should not panic");
    }
    for (id, handle) in handles {
        assert_eq!(handle.get().expect("accepted task should succeed"), id);
    }
    let mut executed_ids = Vec::new();
    while let Ok(id) = executed_rx.recv_timeout(Duration::from_millis(50)) {
        executed_ids.push(id);
    }
    accepted_ids.sort_unstable();
    executed_ids.sort_unstable();
    let terminated = service.wait_termination_timeout(Duration::from_secs(5));
    let rejected_after_close = matches!(
        service.submit_callable(|| Ok::<usize, io::Error>(usize::MAX)),
        Err(SubmissionError::Shutdown)
    );

    assert_eq!(accepted_ids.len() + rejected, SUBMITTERS);
    assert_eq!(executed_ids, accepted_ids);
    assert!(terminated);
    assert!(rejected_after_close);
}

#[test]
fn test_service_admission_multiple_submitters_race_shutdown() {
    test_service_admission_submitters_race_close(false);
}

#[test]
fn test_service_admission_multiple_submitters_race_stop() {
    test_service_admission_submitters_race_close(true);
}
