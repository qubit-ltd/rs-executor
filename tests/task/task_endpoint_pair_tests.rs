// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::io;
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use qubit_executor::CancelResult;
use qubit_executor::TaskExecutionError;
use qubit_executor::TaskStatus;
use qubit_executor::hook::NoopTaskHook;
use qubit_executor::hook::TaskHook;
use qubit_executor::hook::TaskId;
use qubit_executor::task::spi::TaskEndpointPair;

#[derive(Default)]
struct RecordingHook {
    events: Mutex<Vec<String>>,
}

struct PausedAcceptanceHook {
    entered: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
    events: Mutex<Vec<String>>,
}

impl TaskHook for PausedAcceptanceHook {
    fn on_accepted(&self, task_id: TaskId) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push(format!("accepted-enter:{}", task_id.get()));
        let _ignored = self.entered.send(());
        let _ignored = self
            .release
            .lock()
            .expect("release lock should not be poisoned")
            .recv_timeout(Duration::from_secs(5));
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push(format!("accepted-return:{}", task_id.get()));
    }

    fn on_finished(&self, task_id: TaskId, status: TaskStatus) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push(format!("finished:{}:{status:?}", task_id.get()));
    }
}

impl RecordingHook {
    fn events(&self) -> Vec<String> {
        self.events.lock().expect("events lock should not be poisoned").clone()
    }
}

impl TaskHook for RecordingHook {
    fn on_accepted(&self, task_id: TaskId) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push(format!("accepted:{}", task_id.get()));
    }

    fn on_started(&self, task_id: TaskId) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push(format!("started:{}", task_id.get()));
    }

    fn on_finished(&self, task_id: TaskId, status: TaskStatus) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push(format!("finished:{}:{status:?}", task_id.get()));
    }
}

/// Test a completion pair can be created with default and split into usable
/// endpoints.
#[test]
fn test_task_endpoint_pair_default_splits_to_working_endpoints() {
    let pair = TaskEndpointPair::<usize, io::Error>::default();
    let (handle, completion) = pair.into_parts();

    assert!(!handle.is_done());
    assert!(completion.run(|| Ok(42)));
    assert!(handle.is_done());
    assert_eq!(handle.get().expect("completion should publish result"), 42);
}

#[test]
fn test_task_endpoint_pair_with_hook_splits_to_working_endpoints() {
    let pair = TaskEndpointPair::<usize, io::Error>::with_hook(Arc::new(NoopTaskHook));
    let (handle, completion) = pair.into_parts();

    completion.accept();
    assert!(completion.run(|| Ok(42)));
    assert_eq!(handle.get().expect("completion should publish result"), 42);
}

/// Test cancelling an accepted pending task emits a terminal cancellation once.
#[test]
fn test_task_endpoint_pair_cancel_pending_finishes_accepted_task() {
    let hook = Arc::new(RecordingHook::default());
    let pair = TaskEndpointPair::<usize, io::Error>::with_hook(hook.clone());
    let (handle, completion) = pair.into_tracked_parts();

    completion.accept();
    let task_id = handle.task_id().get();

    assert_eq!(handle.cancel(), CancelResult::Cancelled);
    assert_eq!(handle.status(), TaskStatus::Cancelled);
    drop(completion);

    assert_eq!(handle.status(), TaskStatus::Cancelled);
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
    assert_eq!(
        hook.events(),
        vec![format!("accepted:{task_id}"), format!("finished:{task_id}:Cancelled"),],
    );
}

/// Test runner-side cancellation reports cancellation instead of dropped
/// completion.
#[test]
fn test_task_endpoint_pair_cancel_unstarted_slot_finishes_accepted_task() {
    let hook = Arc::new(RecordingHook::default());
    let pair = TaskEndpointPair::<usize, io::Error>::with_hook(hook.clone());
    let (handle, completion) = pair.into_tracked_parts();

    completion.accept();
    let task_id = handle.task_id().get();

    assert!(completion.cancel_unstarted());
    assert_eq!(handle.status(), TaskStatus::Cancelled);
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
    assert_eq!(
        hook.events(),
        vec![format!("accepted:{task_id}"), format!("finished:{task_id}:Cancelled"),],
    );
}

/// Test runner-side cancellation before acceptance does not emit lifecycle
/// hooks.
#[test]
fn test_task_endpoint_pair_cancel_unstarted_before_accept_skips_hooks() {
    let hook = Arc::new(RecordingHook::default());
    let pair = TaskEndpointPair::<usize, io::Error>::with_hook(hook.clone());
    let (handle, completion) = pair.into_tracked_parts();

    assert!(completion.cancel_unstarted());
    assert_eq!(handle.status(), TaskStatus::Cancelled);
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
    assert!(
        hook.events().is_empty(),
        "unaccepted cancellation should not emit lifecycle hooks"
    );
}

/// Test dropping an accepted pending slot publishes `Dropped` without a start
/// event.
#[test]
fn test_task_endpoint_pair_drop_pending_finishes_accepted_task() {
    let hook = Arc::new(RecordingHook::default());
    let pair = TaskEndpointPair::<usize, io::Error>::with_hook(hook.clone());
    let (handle, completion) = pair.into_tracked_parts();

    completion.accept();
    let task_id = handle.task_id().get();
    drop(completion);

    assert_eq!(handle.status(), TaskStatus::Dropped);
    assert!(matches!(handle.get(), Err(TaskExecutionError::Dropped)));
    assert_eq!(
        hook.events(),
        vec![format!("accepted:{task_id}"), format!("finished:{task_id}:Dropped"),],
    );
}

#[test]
fn test_task_endpoint_pair_start_cancel_race_has_one_result_and_one_finished_hook() {
    for _ in 0..128 {
        let hook = Arc::new(RecordingHook::default());
        let (handle, completion) = TaskEndpointPair::<usize, io::Error>::with_hook(hook.clone()).into_tracked_parts();
        completion.accept();
        let barrier = Arc::new(Barrier::new(2));
        let body_calls = Arc::new(AtomicUsize::new(0));
        let worker_barrier = Arc::clone(&barrier);
        let worker_calls = Arc::clone(&body_calls);
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            worker_barrier.wait();
            let ran = completion.run(|| {
                worker_calls.fetch_add(1, Ordering::SeqCst);
                Ok(42)
            });
            done_tx.send(ran).expect("race observer remains alive");
        });
        barrier.wait();
        let cancel = handle.cancel();
        let ran = done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("runner completes race");
        worker.join().expect("race worker finishes");
        if cancel == CancelResult::Cancelled {
            assert!(!ran);
            assert_eq!(body_calls.load(Ordering::SeqCst), 0);
            assert_eq!(handle.status(), TaskStatus::Cancelled);
            assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
        } else {
            assert!(matches!(
                cancel,
                CancelResult::AlreadyRunning | CancelResult::AlreadyFinished
            ));
            assert!(ran);
            assert_eq!(body_calls.load(Ordering::SeqCst), 1);
            assert_eq!(handle.status(), TaskStatus::Succeeded);
            assert_eq!(handle.get().expect("runner publishes success"), 42);
        }
        assert_eq!(
            hook.events()
                .iter()
                .filter(|event| event.starts_with("finished:"))
                .count(),
            1
        );
    }
}

#[test]
fn test_spi_acceptance_callback_pause_allows_concurrent_cancel() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let hook = Arc::new(PausedAcceptanceHook {
        entered: entered_tx,
        release: Mutex::new(release_rx),
        events: Mutex::new(Vec::new()),
    });
    let (handle, slot) = TaskEndpointPair::<usize, io::Error>::with_hook(hook.clone()).into_tracked_parts();
    let task_id = handle.task_id().get();
    let (slot_tx, slot_rx) = mpsc::channel();
    let accept_worker = std::thread::spawn(move || {
        slot.accept();
        slot_tx.send(slot).expect("slot receiver remains alive");
    });

    let entered = entered_rx.recv_timeout(Duration::from_secs(5));
    let (cancel_tx, cancel_rx) = mpsc::channel();
    let cancel_worker = std::thread::spawn(move || {
        cancel_tx.send(handle.cancel()).expect("cancel observer remains alive");
    });
    let cancel = cancel_rx.recv_timeout(Duration::from_secs(5));
    let events_before_release = hook.events.lock().expect("events lock should not be poisoned").clone();
    let _ignored = release_tx.send(());

    let slot = slot_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("acceptance returns after release");
    accept_worker.join().expect("acceptance worker exits");
    cancel_worker.join().expect("cancellation worker exits");
    assert!(entered.is_ok(), "acceptance callback entered");
    assert_eq!(
        cancel.expect("cancel must not wait for acceptance release"),
        CancelResult::Cancelled
    );
    assert!(!slot.run(|| Ok(42)));
    assert_eq!(
        events_before_release,
        vec![
            format!("accepted-enter:{task_id}"),
            format!("finished:{task_id}:Cancelled")
        ],
        "finished may occur after callback entry and before callback return"
    );
    assert_eq!(
        hook.events
            .lock()
            .expect("events lock should not be poisoned")
            .as_slice(),
        [
            format!("accepted-enter:{task_id}"),
            format!("finished:{task_id}:Cancelled"),
            format!("accepted-return:{task_id}")
        ]
    );
}

#[test]
fn test_spi_acceptance_before_handle_publication() {
    for _ in 0..64 {
        let hook = Arc::new(RecordingHook::default());
        let (handle, slot) = TaskEndpointPair::<usize, io::Error>::with_hook(hook.clone()).into_tracked_parts();
        slot.accept();
        let task_id = handle.task_id().get();
        let barrier = Arc::new(Barrier::new(2));
        let body_calls = Arc::new(AtomicUsize::new(0));
        let worker_barrier = Arc::clone(&barrier);
        let worker_calls = Arc::clone(&body_calls);
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            worker_barrier.wait();
            done_tx
                .send(slot.run(|| {
                    worker_calls.fetch_add(1, Ordering::SeqCst);
                    Ok(42)
                }))
                .expect("race observer remains alive");
        });
        barrier.wait();
        let cancel = handle.cancel();
        let ran = done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("runner completes race");
        worker.join().expect("race worker finishes");

        if cancel == CancelResult::Cancelled {
            assert!(!ran);
            assert_eq!(body_calls.load(Ordering::SeqCst), 0);
            assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
        } else {
            assert!(matches!(
                cancel,
                CancelResult::AlreadyRunning | CancelResult::AlreadyFinished
            ));
            assert!(ran);
            assert_eq!(body_calls.load(Ordering::SeqCst), 1);
            assert_eq!(handle.get().expect("runner publishes success"), 42);
        }
        let events = hook.events();
        assert_eq!(events.first(), Some(&format!("accepted:{task_id}")));
        assert_eq!(events.iter().filter(|event| event.starts_with("finished:")).count(), 1);
    }
}
