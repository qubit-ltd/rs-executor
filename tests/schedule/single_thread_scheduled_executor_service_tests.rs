// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`SingleThreadScheduledExecutorService`](qubit_executor::SingleThreadScheduledExecutorService).

use std::future::IntoFuture;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use qubit_executor::CancelResult;
use qubit_executor::ExecutorService;
use qubit_executor::ExecutorServiceBuilderError;
use qubit_executor::ExecutorServiceLifecycle;
use qubit_executor::ScheduledExecutorService;
use qubit_executor::SingleThreadScheduledExecutorService;
use qubit_executor::SubmissionError;
use qubit_executor::TaskExecutionError;
use qubit_executor::TaskStatus;
use qubit_executor::TryGet;
use qubit_executor::task::spi::TaskResultHandle;
use qubit_executor::task::spi::TrackedTaskHandle;

/// Counts when a scheduled closure's captured resource is released.
struct DropProbe(Arc<AtomicUsize>);

impl Drop for DropProbe {
    /// Records destruction of the captured resource.
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
}

struct BlockingPanicOnDrop {
    entered: Option<mpsc::Sender<()>>,
    release: Option<mpsc::Receiver<()>>,
}

struct BlockingDrop {
    entered: Option<mpsc::Sender<()>>,
    release: Option<mpsc::Receiver<()>>,
}

impl Drop for BlockingDrop {
    fn drop(&mut self) {
        self.entered
            .take()
            .expect("observe cleanup entry")
            .send(())
            .expect("test should observe cleanup entry");
        self.release
            .take()
            .expect("release cleanup")
            .recv()
            .expect("test should release cleanup");
    }
}

impl Drop for BlockingPanicOnDrop {
    fn drop(&mut self) {
        self.entered
            .take()
            .expect("drop entry signal")
            .send(())
            .expect("drop test receiver");
        self.release
            .take()
            .expect("drop release receiver")
            .recv()
            .expect("drop test release");
        panic!("scheduled cancellation drop panic");
    }
}

#[test]
fn test_single_thread_scheduled_executor_service_runs_earliest_deadline_first() {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-earliest").expect("scheduled service should start");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    service
        .schedule(Duration::ZERO, move || {
            entered_tx.send(()).expect("signal blocking task start");
            release_rx.recv().expect("receive blocking task release");
            Ok::<(), ()>(())
        })
        .expect("blocking task should schedule");
    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("worker should start blocking task");

    let later = Instant::now();
    let earlier = later
        .checked_sub(Duration::from_secs(1))
        .expect("test deadline should be representable");
    let (sent_tx, sent_rx) = mpsc::channel::<&'static str>();
    let late_tx = sent_tx.clone();
    service
        .schedule_at(later, move || {
            late_tx.send("late").expect("late task should send");
            Ok::<(), ()>(())
        })
        .expect("later deadline should schedule");
    service
        .schedule_at(earlier, move || {
            sent_tx.send("early").expect("early task should send");
            Ok::<(), ()>(())
        })
        .expect("earlier deadline should schedule");

    release_tx.send(()).expect("release blocking task");
    assert_eq!(
        sent_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("early task should run first"),
        "early"
    );
    assert_eq!(
        sent_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("late task should run second"),
        "late"
    );
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_scheduled_termination_does_not_join_concurrent_cancel() {
    let service = Arc::new(
        SingleThreadScheduledExecutorService::new("scheduled-cancel-cleanup").expect("scheduled service should start"),
    );
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let resource = Some(BlockingDrop {
        entered: Some(entered_tx),
        release: Some(release_rx),
    });
    let handle = service
        .schedule_callable(Duration::from_secs(3600), move || {
            let _ = resource.as_ref();
            Ok::<(), ()>(())
        })
        .expect("future task should be accepted");

    let cancel_thread = thread::spawn(move || handle.cancel());
    let cleanup_entered = entered_rx.recv_timeout(Duration::from_secs(5));
    service.shutdown();
    let terminated = service.wait_termination_timeout(Duration::from_secs(5));
    let cancel_finished_before_release = cancel_thread.is_finished();

    let release_result = release_tx.send(());
    let cancel_result = cancel_thread.join();

    assert!(
        cleanup_entered.is_ok(),
        "concurrent cancel should enter captured-value cleanup"
    );
    assert!(release_result.is_ok(), "release captured-value cleanup");
    let cancel_result = cancel_result.expect("cancel call should return");
    assert!(terminated, "service should terminate while cancel cleanup blocks");
    assert!(
        !cancel_finished_before_release,
        "cancel call should still be blocked before cleanup is released"
    );
    assert_eq!(cancel_result, CancelResult::Cancelled);
}

#[test]
fn test_scheduled_stop_recovers_after_captured_drop_panics() {
    let service = Arc::new(
        SingleThreadScheduledExecutorService::new("test-stop-drop-panic").expect("scheduled service should start"),
    );
    let (task_entered_tx, task_entered_rx) = mpsc::channel();
    let (task_release_tx, task_release_rx) = mpsc::channel();
    service
        .schedule(Duration::ZERO, move || {
            task_entered_tx.send(()).expect("running task signal");
            task_release_rx.recv().expect("running task release");
            Ok::<(), ()>(())
        })
        .expect("blocking task should schedule");
    task_entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("worker should start blocking task");

    let (drop_entered_tx, drop_entered_rx) = mpsc::channel();
    let (drop_release_tx, drop_release_rx) = mpsc::channel();
    let probe = BlockingPanicOnDrop {
        entered: Some(drop_entered_tx),
        release: Some(drop_release_rx),
    };
    let panic_handle = service
        .schedule_callable(Duration::from_secs(60), move || {
            let _ = &probe;
            Ok::<(), ()>(())
        })
        .expect("panicking drop task should schedule");
    let first_normal = service
        .schedule_callable(Duration::from_secs(60), || Ok::<usize, ()>(1))
        .expect("first normal task should schedule");
    let second_normal = service
        .schedule_callable(Duration::from_secs(60), || Ok::<usize, ()>(2))
        .expect("second normal task should schedule");

    let stop_service = Arc::clone(&service);
    let stop_thread = thread::spawn(move || std::panic::catch_unwind(AssertUnwindSafe(|| stop_service.stop())));
    drop_entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("stop should begin dropping the captured resource");
    let terminated_while_drop_blocked = service.is_terminated();
    drop_release_tx.send(()).expect("release panicking drop");
    assert!(stop_thread.join().expect("stop thread should return").is_err());

    assert!(!terminated_while_drop_blocked);
    assert!(matches!(panic_handle.get(), Err(TaskExecutionError::Cancelled)));
    assert!(matches!(first_normal.get(), Err(TaskExecutionError::Cancelled)));
    assert!(matches!(second_normal.get(), Err(TaskExecutionError::Cancelled)));
    task_release_tx.send(()).expect("release running task");
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
    assert_eq!(service.lifecycle(), ExecutorServiceLifecycle::Terminated);
}

#[test]
fn test_single_thread_scheduled_executor_service_schedule_callable_returns_result() {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-callable").expect("scheduled service should start");

    let handle = service
        .schedule_callable(Duration::from_millis(10), || Ok::<usize, ()>(40 + 2))
        .expect("callable should schedule");

    assert_eq!(handle.get().expect("callable should succeed"), 42);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_rejects_unrepresentable_delay() {
    let service = SingleThreadScheduledExecutorService::new("test-scheduled-invalid-deadline")
        .expect("scheduled service should start");
    let result = service.schedule_callable(Duration::MAX, || Ok::<(), ()>(()));
    assert!(matches!(result, Err(SubmissionError::InvalidDeadline)));
    assert_eq!(service.queued_count(), 0);
    let handle = service
        .schedule_callable(Duration::ZERO, || Ok::<usize, ()>(42))
        .expect("service should remain usable after rejecting a deadline");
    assert_eq!(handle.get().expect("follow-up task should succeed"), 42);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_wait_termination_timeout_handles_max_duration() {
    let service =
        SingleThreadScheduledExecutorService::new("test-termination-overflow").expect("scheduled service should start");
    service.shutdown();
    service.wait_termination();

    assert!(service.wait_termination_timeout(Duration::MAX));
}

#[test]
fn test_scheduled_service_max_timeout_waits_for_shutdown() {
    let service = Arc::new(
        SingleThreadScheduledExecutorService::new("test-max-timeout-running").expect("scheduled service should start"),
    );
    let shutdown_service = service.clone();
    let shutdown_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        shutdown_service.shutdown();
    });

    assert!(service.wait_termination_timeout(Duration::MAX));
    shutdown_thread.join().expect("shutdown thread should not panic");
}

#[test]
fn test_single_thread_scheduled_executor_service_schedule_at_runs_runnable() {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-at").expect("scheduled service should start");
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
fn test_scheduled_deadline_survives_shutdown() {
    let service = SingleThreadScheduledExecutorService::new("deadline-shutdown").expect("scheduler starts");
    let deadline = Instant::now() + Duration::from_millis(100);
    let (tx, rx) = mpsc::channel();
    let handle = service
        .schedule_callable_at(deadline, move || {
            tx.send(Instant::now()).expect("observer exists");
            Ok::<(), ()>(())
        })
        .expect("submission succeeds");
    service.shutdown();

    let observed = rx.recv_timeout(Duration::from_secs(5));
    if observed.is_err() {
        let _ = service.stop();
    }
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
    let started = observed.expect("accepted task runs after shutdown");
    assert!(started >= deadline, "task must not start before its deadline");
    assert!(handle.get().is_ok(), "accepted task should complete successfully");
}

#[test]
fn test_single_thread_scheduled_executor_service_submit_callable_runs_immediately() {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-submit").expect("scheduled service should start");

    let handle = service
        .submit_callable(|| Ok::<usize, ()>(6 * 7))
        .expect("callable should submit");

    assert_eq!(handle.get().expect("callable should succeed"), 42);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_submit_runs_runnable() {
    let service = SingleThreadScheduledExecutorService::new("test-scheduled-submit-runnable")
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
fn test_single_thread_scheduled_executor_service_submit_tracked_callable_reports_status() {
    let service = SingleThreadScheduledExecutorService::new("test-scheduled-submit-tracked")
        .expect("scheduled service should start");

    let handle = service
        .submit_tracked_callable(|| Ok::<usize, ()>(42))
        .expect("tracked callable should submit");

    assert_eq!(handle.get().expect("tracked callable should succeed"), 42);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_counts_queued_and_running_tasks() {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-counts").expect("scheduled service should start");
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

    release_tx.send(()).expect("task should receive release signal");
    assert_eq!(handle.get().expect("task should complete"), 42);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_handle_observation_paths() {
    let service = SingleThreadScheduledExecutorService::new("test-scheduled-handle-observation")
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
    let service = SingleThreadScheduledExecutorService::new("test-scheduled-handle-ready")
        .expect("scheduled service should start");
    let handle = service
        .schedule_callable(Duration::ZERO, || Ok::<usize, ()>(42))
        .expect("task should schedule");

    service.shutdown();
    assert!(
        service.wait_termination_timeout(Duration::from_secs(5)),
        "scheduled service should terminate after publishing the task result"
    );
    match TaskResultHandle::try_get(handle) {
        TryGet::Ready(result) => {
            assert_eq!(result.expect("task should succeed"), 42);
        }
        TryGet::Pending(_) => panic!("terminated service should have published the task result"),
    }
}

#[tokio::test]
async fn test_single_thread_scheduled_executor_service_handle_await_returns_result() {
    let service = SingleThreadScheduledExecutorService::new("test-scheduled-handle-await")
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
        SingleThreadScheduledExecutorService::new("test-scheduled-cancel").expect("scheduled service should start");
    let timeout = Duration::from_secs(5);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let blocker = service
        .schedule(Duration::ZERO, move || {
            entered_tx.send(()).expect("test should observe blocker entry");
            release_rx.recv_timeout(timeout).expect("test should release blocker");
            Ok::<(), ()>(())
        })
        .expect("blocker should schedule");
    entered_rx.recv_timeout(timeout).expect("blocker should start");

    let (sent_tx, sent_rx) = mpsc::channel::<()>();
    let handle = service
        .schedule(Duration::ZERO, move || {
            sent_tx.send(()).expect("test should retain receiver");
            Ok::<(), ()>(())
        })
        .expect("target should schedule behind blocker");
    let first = handle.cancel();
    let second = handle.cancel();
    let released = release_tx.send(());
    service.shutdown();

    let terminated = service.wait_termination_timeout(timeout);

    assert!(released.is_ok(), "blocker should be waiting for release");
    assert!(terminated, "scheduled service should terminate");
    assert_eq!(first, CancelResult::Cancelled);
    assert_eq!(second, CancelResult::AlreadyFinished);
    blocker.get().expect("blocker should finish normally");
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
    assert!(
        matches!(sent_rx.try_recv(), Err(mpsc::TryRecvError::Disconnected)),
        "cancelled callable must be destroyed without sending"
    );
    assert!(service.is_terminated());
}

/// Verifies cancellation removes a pending entry and releases its closure
/// promptly.
#[test]
fn test_single_thread_scheduled_executor_service_cancel_releases_queued_entry() {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-release").expect("scheduled service should start");
    let drops = Arc::new(AtomicUsize::new(0));
    let mut probe = Some(DropProbe(Arc::clone(&drops)));

    let handle = service
        .schedule(Duration::from_secs(30), move || {
            drop(probe.take());
            Ok::<(), ()>(())
        })
        .expect("delayed task should schedule");

    assert_eq!(handle.cancel(), CancelResult::Cancelled);
    assert_eq!(service.queued_count(), 0);
    assert_eq!(drops.load(Ordering::Acquire), 1);
    service.shutdown();
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_stop_cancels_pending_task() {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-stop").expect("scheduled service should start");
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
        SingleThreadScheduledExecutorService::new("test-scheduled-reject").expect("scheduled service should start");

    service.shutdown();
    assert!(service.is_not_running());

    assert!(matches!(
        service.schedule(Duration::ZERO, || Ok::<(), ()>(())),
        Err(SubmissionError::Shutdown)
    ));
    service.wait_termination();
}

#[test]
fn test_single_thread_scheduled_executor_service_reports_shutting_down_with_running_work() {
    let service =
        SingleThreadScheduledExecutorService::new("test-scheduled-lifecycle").expect("scheduled service should start");
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    service
        .schedule(Duration::ZERO, move || {
            started_tx.send(()).expect("test should receive task start signal");
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

    release_tx.send(()).expect("task should receive release signal");
    service.wait_termination();
    assert_eq!(service.lifecycle(), ExecutorServiceLifecycle::Terminated);
}

#[test]
fn test_single_thread_scheduled_executor_service_reports_spawn_failure() {
    let result =
        SingleThreadScheduledExecutorService::with_stack_size("test-scheduled-spawn-failure", Some(usize::MAX));

    assert!(matches!(result, Err(ExecutorServiceBuilderError::SpawnWorker { .. })));
}
