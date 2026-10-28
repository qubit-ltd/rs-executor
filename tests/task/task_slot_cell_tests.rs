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
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;

use qubit_executor::CancelResult;
use qubit_executor::TaskExecutionError;
use qubit_executor::task::spi::TaskEndpointPair;
use qubit_executor::task::spi::TaskSlotCell;

/// Verifies that a failed start returns ownership to the shared slot cell.
#[test]
fn test_task_slot_cell_try_start_restores_cancelled_slot() {
    let (handle, slot) = TaskEndpointPair::<usize, io::Error>::new().into_tracked_parts();
    let cell = TaskSlotCell::new(slot);
    cell.accept();
    assert_eq!(handle.cancel(), CancelResult::Cancelled);

    assert!(cell.try_start().is_none());
    assert!(cell.take().is_some());
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
}

/// Verifies that shared cancellation consumes an unstarted task only once.
#[test]
fn test_task_slot_cell_cancel_unstarted_publishes_cancellation() {
    let (handle, slot) = TaskEndpointPair::<usize, io::Error>::new().into_tracked_parts();
    let cell = TaskSlotCell::new(slot);
    cell.accept();

    assert!(cell.cancel_unstarted());
    assert!(!cell.cancel_unstarted());
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
}

#[test]
fn test_task_slot_cell_successful_start_consumes_slot() {
    let (handle, slot) = TaskEndpointPair::<usize, io::Error>::new().into_parts();
    let cell = TaskSlotCell::new(slot);
    cell.accept();
    let running = match cell.try_start() {
        Some(running) => running,
        None => panic!("pending task should start"),
    };
    assert!(running.complete(Ok(42)));
    assert!(cell.take().is_none());
    assert!(cell.try_start().is_none());
    assert!(!cell.cancel_unstarted());
    assert_eq!(handle.get().expect("started task should complete"), 42);
}

#[test]
fn test_task_slot_cell_take_races_with_cancel() {
    let (handle, slot) = TaskEndpointPair::<usize, io::Error>::new().into_tracked_parts();
    let cell = Arc::new(TaskSlotCell::new(slot));
    cell.accept();
    let barrier = Arc::new(Barrier::new(2));
    let calls = Arc::new(AtomicUsize::new(0));
    let worker_cell = Arc::clone(&cell);
    let worker_barrier = Arc::clone(&barrier);
    let worker_calls = Arc::clone(&calls);
    let worker = thread::spawn(move || {
        worker_barrier.wait();
        worker_cell.take().is_some_and(|slot| {
            slot.run(|| {
                worker_calls.fetch_add(1, Ordering::AcqRel);
                Ok(42)
            })
        })
    });
    let cancel_cell = Arc::clone(&cell);
    let cancel_barrier = Arc::clone(&barrier);
    let canceller = thread::spawn(move || {
        cancel_barrier.wait();
        cancel_cell.cancel_unstarted()
    });

    let taken_and_run = worker.join().expect("worker should finish");
    let cancelled = canceller.join().expect("canceller should finish");
    if taken_and_run {
        assert!(!cancelled);
        assert!(matches!(handle.get(), Ok(42)));
        assert_eq!(calls.load(Ordering::Acquire), 1);
    } else {
        assert!(cancelled);
        assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
        assert_eq!(calls.load(Ordering::Acquire), 0);
    }
    assert!(cell.take().is_none());
    assert!(cell.try_start().is_none());
    assert!(!cell.cancel_unstarted());
}

#[test]
fn test_task_slot_cell_start_races_with_cancel() {
    let (handle, slot) = TaskEndpointPair::<usize, io::Error>::new().into_tracked_parts();
    let cell = Arc::new(TaskSlotCell::new(slot));
    cell.accept();
    let barrier = Arc::new(Barrier::new(2));
    let calls = Arc::new(AtomicUsize::new(0));
    let worker_cell = Arc::clone(&cell);
    let worker_barrier = Arc::clone(&barrier);
    let worker_calls = Arc::clone(&calls);
    let worker = thread::spawn(move || {
        worker_barrier.wait();
        worker_cell.try_start().is_some_and(|running| {
            running.run(|| {
                worker_calls.fetch_add(1, Ordering::AcqRel);
                Ok(42)
            })
        })
    });
    let cancel_cell = Arc::clone(&cell);
    let cancel_barrier = Arc::clone(&barrier);
    let canceller = thread::spawn(move || {
        cancel_barrier.wait();
        cancel_cell.cancel_unstarted()
    });

    let started = worker.join().expect("worker should finish");
    let cancelled = canceller.join().expect("canceller should finish");
    let result = handle.get();
    if started {
        assert!(!cancelled);
        assert!(matches!(result, Ok(42)));
        assert_eq!(calls.load(Ordering::Acquire), 1);
    } else {
        assert!(cancelled);
        assert!(matches!(result, Err(TaskExecutionError::Cancelled)));
        assert_eq!(calls.load(Ordering::Acquire), 0);
    }
    assert!(cell.take().is_none());
    assert!(cell.try_start().is_none());
    assert!(!cell.cancel_unstarted());
}
