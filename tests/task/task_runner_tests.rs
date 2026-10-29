// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for task runner utilities.

use std::io;
use std::panic;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use qubit_executor::TaskExecutionError;
use qubit_executor::TaskStatus;
use qubit_executor::hook::TaskHook;
use qubit_executor::hook::TaskId;
use qubit_executor::task::spi::TaskEndpointPair;
use qubit_executor::task::spi::TaskRunner;

struct PanicOnDrop;

impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        panic!("drop panic");
    }
}

#[derive(Default)]
struct FinishCounter {
    accepted: AtomicUsize,
    started: AtomicUsize,
    finished: AtomicUsize,
    status: std::sync::Mutex<Option<TaskStatus>>,
}

impl TaskHook for FinishCounter {
    fn on_accepted(&self, _: TaskId) {
        self.accepted.fetch_add(1, Ordering::SeqCst);
    }

    fn on_started(&self, _: TaskId) {
        self.started.fetch_add(1, Ordering::SeqCst);
    }

    fn on_finished(&self, _: TaskId, status: TaskStatus) {
        *self.status.lock().expect("status lock should not be poisoned") = Some(status);
        self.finished.fetch_add(1, Ordering::SeqCst);
    }
}

struct PanicsWithSecondaryPayload;

impl Drop for PanicsWithSecondaryPayload {
    fn drop(&mut self) {
        panic::panic_any(SecondaryPayload);
    }
}

struct SecondaryPayload;

impl Drop for SecondaryPayload {
    fn drop(&mut self) {
        panic!("secondary payload drop panic");
    }
}

fn assert_finished_once(hook: &FinishCounter, status: TaskStatus) {
    assert_eq!(hook.accepted.load(Ordering::SeqCst), 1);
    assert_eq!(hook.started.load(Ordering::SeqCst), 1);
    assert_eq!(hook.finished.load(Ordering::SeqCst), 1);
    assert_eq!(
        *hook.status.lock().expect("status lock should not be poisoned"),
        Some(status)
    );
}

#[test]
fn test_runner_converts_callable_drop_panic() {
    let probe = PanicOnDrop;
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        TaskRunner::new(move || {
            let _keep = &probe;
            Ok::<usize, io::Error>(42)
        })
        .call::<usize, io::Error>()
    }));

    assert!(matches!(
        result.expect("callable drop panic should not escape"),
        Err(TaskExecutionError::Panicked),
    ));
}

#[test]
fn test_runner_converts_payload_drop_panic() {
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        TaskRunner::new(|| -> Result<usize, io::Error> { panic::panic_any(PanicOnDrop) }).call::<usize, io::Error>()
    }));

    assert!(matches!(
        result.expect("panic payload drop panic should not escape"),
        Err(TaskExecutionError::Panicked),
    ));
}

#[test]
fn test_runner_run_publishes_panicked_when_callable_drop_panics() {
    let probe = PanicOnDrop;
    let (handle, completion) = TaskEndpointPair::<usize, io::Error>::new().into_parts();

    TaskRunner::new(move || {
        let _keep = &probe;
        Ok::<usize, io::Error>(42)
    })
    .run(completion);

    assert!(matches!(handle.get(), Err(TaskExecutionError::Panicked)));
}

#[test]
fn test_runner_run_started_publishes_panicked_when_payload_drop_panics() {
    let (handle, completion) = TaskEndpointPair::<usize, io::Error>::new().into_parts();
    let running = match completion.try_start() {
        Ok(running) => running,
        Err(_) => panic!("pending completion should start"),
    };

    TaskRunner::new(|| -> Result<usize, io::Error> { panic::panic_any(PanicOnDrop) }).run_started(running);

    assert!(matches!(handle.get(), Err(TaskExecutionError::Panicked)));
}

#[test]
fn test_runner_executes_through_completion() {
    let (handle, completion) = TaskEndpointPair::<usize, io::Error>::new().into_parts();

    TaskRunner::new(|| Ok::<usize, io::Error>(42)).run(completion);

    assert_eq!(handle.get().expect("runner should publish result"), 42);
}

#[test]
fn test_runner_executes_through_running_slot() {
    let (handle, completion) = TaskEndpointPair::<usize, io::Error>::new().into_parts();
    let running = match completion.try_start() {
        Ok(running) => running,
        Err(_) => panic!("pending completion should start"),
    };

    TaskRunner::new(|| Ok::<usize, io::Error>(42)).run_started(running);

    assert_eq!(handle.get().expect("runner should publish result"), 42);
}

#[test]
fn test_runner_run_detached_discards_result() {
    TaskRunner::new(|| Ok::<usize, io::Error>(42)).run_detached();
}

#[test]
fn test_runner_finishes_when_unreceived_result_drop_panics() {
    let hook = Arc::new(FinishCounter::default());
    let (handle, slot) = TaskEndpointPair::<PanicOnDrop, ()>::with_hook(hook.clone()).into_parts();
    slot.accept();
    drop(handle);

    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        TaskRunner::new(|| Ok::<PanicOnDrop, ()>(PanicOnDrop)).run(slot)
    }));

    assert!(outcome.expect("result cleanup panic must be contained"));
    assert_finished_once(&hook, TaskStatus::Succeeded);
}

#[test]
fn test_runner_finishes_when_unreceived_error_drop_panics() {
    let hook = Arc::new(FinishCounter::default());
    let (handle, slot) = TaskEndpointPair::<(), PanicOnDrop>::with_hook(hook.clone()).into_parts();
    slot.accept();
    let running = match slot.try_start() {
        Ok(running) => running,
        Err(_) => panic!("accepted pending task should start"),
    };
    drop(handle);

    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        TaskRunner::new(|| Err::<(), PanicOnDrop>(PanicOnDrop)).run_started(running)
    }));

    assert!(outcome.expect("error cleanup panic must be contained"));
    assert_finished_once(&hook, TaskStatus::Failed);
}

#[test]
fn test_runner_detached_contains_success_result_drop_panics() {
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        TaskRunner::new(|| Ok::<PanicOnDrop, ()>(PanicOnDrop)).run_detached();
    }));

    outcome.expect("detached success result cleanup panic must be contained");
}

#[test]
fn test_runner_detached_contains_error_drop_panics_with_secondary_payload() {
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        TaskRunner::new(|| Err::<(), PanicsWithSecondaryPayload>(PanicsWithSecondaryPayload)).run_detached();
    }));

    if let Err(payload) = outcome {
        // Keep the secondary-panic payload from panicking during test failure.
        std::mem::forget(payload);
        panic!("detached error cleanup panics must be contained");
    }
}
