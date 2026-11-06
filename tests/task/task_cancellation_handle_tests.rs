// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;

use qubit_executor::CancelResult;
use qubit_executor::TaskCancellationHandle;
use qubit_executor::TaskExecutionError;
use qubit_executor::TaskStatus;
use qubit_executor::task::spi::TaskEndpointPair;

fn assert_send_sync_clone<T: Send + Sync + Clone>() {}

#[test]
fn test_cancellation_handle_is_send_sync_and_clone() {
    assert_send_sync_clone::<TaskCancellationHandle>();
}

#[test]
fn test_cancellation_handle_cancels_pending_task() {
    let (task, slot) = TaskEndpointPair::<usize, ()>::new().into_tracked_parts();
    let cancellation = task.cancellation_handle();
    let second = cancellation.clone();
    assert_eq!(cancellation.cancel(), CancelResult::Cancelled);
    assert_eq!(second.cancel(), CancelResult::AlreadyFinished);
    assert!(!slot.run(|| Ok(42)));
    assert!(matches!(task.get(), Err(TaskExecutionError::Cancelled)));
}

#[test]
fn test_cancellation_handle_reports_running_and_finished_tasks() {
    let (task, slot) = TaskEndpointPair::<usize, ()>::new().into_tracked_parts();
    let cancellation = task.cancellation_handle();
    let running = match slot.try_start() {
        Ok(running) => running,
        Err(_) => panic!("task should start"),
    };
    assert_eq!(cancellation.cancel(), CancelResult::AlreadyRunning);
    assert!(running.run(|| Ok(42)));
    assert_eq!(cancellation.cancel(), CancelResult::AlreadyFinished);
    assert_eq!(task.status(), TaskStatus::Succeeded);
}

#[test]
fn test_dropping_cancellation_handle_does_not_cancel() {
    let (task, slot) = TaskEndpointPair::<usize, ()>::new().into_tracked_parts();
    drop(task.cancellation_handle());
    assert_eq!(task.status(), TaskStatus::Pending);
    assert!(slot.run(|| Ok(42)));
    assert_eq!(task.get().expect("task should finish"), 42);
}

#[test]
fn test_cancellation_handle_and_legacy_cancel_have_one_winner() {
    let (task, slot) = TaskEndpointPair::<usize, ()>::new().into_tracked_parts();
    let endpoint = task.cancellation_handle();
    let endpoint_clone = endpoint.clone();
    let barrier = Arc::new(Barrier::new(3));
    let first_barrier = Arc::clone(&barrier);
    let first = thread::spawn(move || {
        first_barrier.wait();
        endpoint.cancel()
    });
    let second_barrier = Arc::clone(&barrier);
    let second = thread::spawn(move || {
        second_barrier.wait();
        endpoint_clone.cancel()
    });
    barrier.wait();
    let legacy = task.cancel();
    let results = [first.join().unwrap(), second.join().unwrap(), legacy];
    assert_eq!(results.iter().filter(|&&r| r == CancelResult::Cancelled).count(), 1);
    assert_eq!(
        results.iter().filter(|&&r| r == CancelResult::AlreadyFinished).count(),
        2
    );
    assert!(!slot.run(|| Ok(42)));
}

#[test]
fn test_cancellation_adapter_cleanup_runs_only_for_the_concurrent_winner() {
    struct DropSignal(Arc<AtomicUsize>);
    impl Drop for DropSignal {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let (task, slot) = TaskEndpointPair::<usize, ()>::new().into_tracked_parts();
    let inner = task.cancellation_handle();
    let cleanup_count = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let signal = DropSignal(Arc::clone(&drops));
    let cleanup = Arc::clone(&cleanup_count);
    let adapted = TaskCancellationHandle::new(move || {
        let _ = &signal;
        let result = inner.cancel();
        if result == CancelResult::Cancelled {
            cleanup.fetch_add(1, Ordering::SeqCst);
        }
        result
    });
    let clone = adapted.clone();
    let main_thread_clone = adapted.clone();
    let barrier = Arc::new(Barrier::new(3));
    let first_barrier = Arc::clone(&barrier);
    let first = thread::spawn(move || {
        first_barrier.wait();
        adapted.cancel()
    });
    let second_barrier = Arc::clone(&barrier);
    let second = thread::spawn(move || {
        second_barrier.wait();
        clone.cancel()
    });
    barrier.wait();
    let main_thread_result = main_thread_clone.cancel();
    drop(main_thread_clone);
    let results = [
        first.join().unwrap(),
        second.join().unwrap(),
        main_thread_result,
    ];
    assert_eq!(
        results
            .iter()
            .filter(|&&result| result == CancelResult::Cancelled)
            .count(),
        1
    );
    assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
    assert!(!slot.run(|| Ok(42)));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn test_cancellation_adapter_cleanup_is_skipped_when_raw_cancel_wins_first() {
    let (task, slot) = TaskEndpointPair::<usize, ()>::new().into_tracked_parts();
    let inner = task.cancellation_handle();
    let cleanup_count = Arc::new(AtomicUsize::new(0));
    let cleanup = Arc::clone(&cleanup_count);
    let adapted = TaskCancellationHandle::new(move || {
        let result = inner.cancel();
        if result == CancelResult::Cancelled {
            cleanup.fetch_add(1, Ordering::SeqCst);
        }
        result
    });

    assert_eq!(task.cancel(), CancelResult::Cancelled);
    assert_eq!(adapted.cancel(), CancelResult::AlreadyFinished);
    assert_eq!(cleanup_count.load(Ordering::SeqCst), 0);
    assert!(!slot.run(|| Ok(42)));
}

#[test]
fn test_cancellation_callback_panic_propagates_to_caller() {
    let cancellation = TaskCancellationHandle::new(|| -> CancelResult { panic!("cancellation callback panic") });
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cancellation.cancel()));
    assert!(matches!(result, Err(payload) if payload.downcast_ref::<&str>() == Some(&"cancellation callback panic")));
}
