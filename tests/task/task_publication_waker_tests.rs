// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression tests for result publication when a registered Waker panics.

use std::future::Future;
use std::future::IntoFuture;
use std::io;
use std::panic;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use std::task::Wake;
use std::task::Waker;
use std::time::Duration;

use qubit_executor::TaskExecutionError;
use qubit_executor::TaskStatus;
use qubit_executor::hook::TaskHook;
use qubit_executor::hook::TaskId;
use qubit_executor::task::spi::TaskEndpointPair;
use qubit_executor::task::spi::TaskSlot;

#[derive(Default)]
struct FinishCounter {
    accepted: AtomicUsize,
    started: AtomicUsize,
    finished: AtomicUsize,
    status: Mutex<Option<TaskStatus>>,
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

struct PanicWake {
    invoked: Arc<AtomicBool>,
}

impl Wake for PanicWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.invoked.store(true, Ordering::SeqCst);
        panic!("registered result waker panic");
    }
}

fn assert_hook(hook: &FinishCounter, started: usize, status: TaskStatus) {
    assert_eq!(hook.accepted.load(Ordering::SeqCst), 1);
    assert_eq!(hook.started.load(Ordering::SeqCst), started);
    assert_eq!(hook.finished.load(Ordering::SeqCst), 1);
    assert_eq!(
        *hook.status.lock().expect("status lock should not be poisoned"),
        Some(status)
    );
}

fn assert_subsequent_task_works() {
    let (handle, slot) = TaskEndpointPair::<usize, io::Error>::new().into_parts();
    assert!(slot.run(|| Ok(42)));
    assert_eq!(handle.get().expect("subsequent task should complete"), 42);
}

fn assert_panic_waker_path(action: impl FnOnce(TaskSlot<usize, io::Error>), status: TaskStatus, started: usize) {
    let hook = Arc::new(FinishCounter::default());
    let (handle, slot) = TaskEndpointPair::<usize, io::Error>::with_hook(hook.clone()).into_tracked_parts();
    slot.accept();
    let mut future = Box::pin(handle.into_future());
    let invoked = Arc::new(AtomicBool::new(false));
    let waker = Waker::from(Arc::new(PanicWake {
        invoked: Arc::clone(&invoked),
    }));
    let mut context = Context::from_waker(&waker);
    assert!(Future::poll(future.as_mut(), &mut context).is_pending());

    let outcome = panic::catch_unwind(AssertUnwindSafe(|| action(slot)));
    assert!(outcome.is_ok(), "publication must contain the Waker panic");
    assert!(invoked.load(Ordering::SeqCst), "registered Waker must be invoked");
    assert_hook(&hook, started, status);

    let normal_waker = Waker::noop();
    let mut normal_context = Context::from_waker(normal_waker);
    let result = Future::poll(future.as_mut(), &mut normal_context);
    match status {
        TaskStatus::Succeeded => assert!(matches!(result, Poll::Ready(Ok(42)))),
        TaskStatus::Cancelled => assert!(matches!(result, Poll::Ready(Err(TaskExecutionError::Cancelled)))),
        TaskStatus::Dropped => assert!(matches!(result, Poll::Ready(Err(TaskExecutionError::Dropped)))),
        _ => panic!("unexpected terminal status {status:?}"),
    }
    assert_subsequent_task_works();
}

#[test]
fn test_completion_contains_waker_panic() {
    assert_panic_waker_path(|slot| assert!(slot.run(|| Ok(42))), TaskStatus::Succeeded, 1);
}

#[test]
fn test_cancellation_contains_waker_panic() {
    assert_panic_waker_path(|slot| assert!(slot.cancel_unstarted()), TaskStatus::Cancelled, 0);
}

#[test]
fn test_pending_drop_contains_waker_panic() {
    assert_panic_waker_path(drop, TaskStatus::Dropped, 0);
}

#[test]
fn test_running_drop_contains_waker_panic() {
    let hook = Arc::new(FinishCounter::default());
    let (handle, slot) = TaskEndpointPair::<usize, io::Error>::with_hook(hook.clone()).into_tracked_parts();
    slot.accept();
    let running = slot.try_start().ok().expect("pending task should start");
    let mut future = Box::pin(handle.into_future());
    let invoked = Arc::new(AtomicBool::new(false));
    let waker = Waker::from(Arc::new(PanicWake {
        invoked: Arc::clone(&invoked),
    }));
    let mut context = Context::from_waker(&waker);
    assert!(Future::poll(future.as_mut(), &mut context).is_pending());

    let outcome = panic::catch_unwind(AssertUnwindSafe(|| drop(running)));
    assert!(outcome.is_ok(), "running endpoint drop must contain the Waker panic");
    assert!(invoked.load(Ordering::SeqCst), "registered Waker must be invoked");
    assert_hook(&hook, 1, TaskStatus::Dropped);
    assert!(matches!(
        Future::poll(future.as_mut(), &mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(TaskExecutionError::Dropped)),
    ));
    assert_subsequent_task_works();
}

struct ProviderReenterWake {
    provider_slot: Arc<Mutex<Option<TaskSlot<usize, ()>>>>,
    invoked: Arc<AtomicBool>,
    returned: Arc<AtomicBool>,
}

impl Wake for ProviderReenterWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.invoked.store(true, Ordering::SeqCst);
        let _guard = self.provider_slot.lock().expect("provider lock should be available");
        self.returned.store(true, Ordering::SeqCst);
    }
}

#[test]
fn test_endpoint_drop_publishes_outside_provider_lock() {
    const MARKER: &str = "task_publication_waker_provider_lock";
    let result = crate::internal::process_watchdog::run_test(
        "task::task_publication_waker_tests::test_endpoint_drop_publishes_outside_provider_lock",
        MARKER,
        Duration::from_secs(15),
        || {
            let (handle, slot) = TaskEndpointPair::<usize, ()>::new().into_parts();
            let mut future = Box::pin(handle.into_future());
            let provider_slot = Arc::new(Mutex::new(Some(slot)));
            let invoked = Arc::new(AtomicBool::new(false));
            let returned = Arc::new(AtomicBool::new(false));
            let waker = Waker::from(Arc::new(ProviderReenterWake {
                provider_slot: Arc::clone(&provider_slot),
                invoked: Arc::clone(&invoked),
                returned: Arc::clone(&returned),
            }));
            let mut context = Context::from_waker(&waker);
            assert!(Future::poll(future.as_mut(), &mut context).is_pending());

            let detached = {
                let mut guard = provider_slot.lock().expect("provider lock should be available");
                guard.take().expect("provider owns endpoint")
            };
            drop(detached);
            assert!(invoked.load(Ordering::SeqCst));
            assert!(returned.load(Ordering::SeqCst));
            assert!(matches!(
                Future::poll(future.as_mut(), &mut Context::from_waker(Waker::noop())),
                Poll::Ready(Err(TaskExecutionError::Dropped)),
            ));
        },
    );
    assert!(result.is_ok(), "watchdog should complete: {result:?}");
}
