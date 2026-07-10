// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Internal test support for scheduled executor services.
//!
//! This module is hidden from public documentation and exists so integration
//! tests can exercise scheduler internals that are deliberately not part of the
//! user-facing API.

use std::{
    cmp::Ordering as CompareOrdering,
    panic::{
        self,
        AssertUnwindSafe,
    },
    sync::Arc,
    time::{
        Duration,
        Instant,
    },
};

use qubit_atomic::Atomic;

use crate::{
    CancelResult,
    TaskExecutionError,
    task::spi::TaskEndpointPair,
};

use super::{
    completable_scheduled_task::CompletableScheduledTask,
    scheduled_task::ScheduledTask,
    scheduled_task_entry::{
        ScheduledTaskEntry,
        StartedScheduledTask,
    },
    single_thread_scheduled_executor_service_inner::SingleThreadScheduledExecutorServiceInner,
};

struct NoopEntry;

impl ScheduledTaskEntry for NoopEntry {
    fn accept(&self) {}

    fn is_cancelled(&self) -> bool {
        false
    }

    fn start(self: Box<Self>) -> Option<StartedScheduledTask> {
        Some(Box::new(|| {}))
    }

    fn cancel(self: Box<Self>) -> bool {
        true
    }
}

struct CancelRejectedEntry;

impl ScheduledTaskEntry for CancelRejectedEntry {
    fn accept(&self) {}

    fn is_cancelled(&self) -> bool {
        false
    }

    fn start(self: Box<Self>) -> Option<StartedScheduledTask> {
        None
    }

    fn cancel(self: Box<Self>) -> bool {
        false
    }
}

fn scheduled_task(deadline: Instant, sequence: usize) -> ScheduledTask {
    ScheduledTask::new(deadline, sequence, Box::new(NoopEntry))
}

/// Verifies heap ordering helpers for scheduled task entries.
pub fn verify_scheduled_task_ordering() {
    let deadline = Instant::now();
    let noop_entry = NoopEntry;
    noop_entry.accept();
    assert!(!noop_entry.is_cancelled());
    let started_task = Box::new(NoopEntry)
        .start()
        .expect("noop entry should produce a runnable task");
    started_task();
    assert!(Box::new(NoopEntry).cancel());
    let rejected_entry = CancelRejectedEntry;
    rejected_entry.accept();
    assert!(!rejected_entry.is_cancelled());
    assert!(Box::new(CancelRejectedEntry).start().is_none());
    assert!(!Box::new(CancelRejectedEntry).cancel());

    assert!(scheduled_task(deadline, 7) == scheduled_task(deadline, 7));
    assert!(
        scheduled_task(deadline, 7)
            != scheduled_task(deadline + Duration::from_millis(1), 7)
    );
    assert!(scheduled_task(deadline, 7) != scheduled_task(deadline, 8));
    assert!(scheduled_task(deadline, 0) > scheduled_task(deadline, 1));
    assert!(
        scheduled_task(deadline, 0)
            > scheduled_task(deadline + Duration::from_millis(1), 0)
    );
    assert_eq!(
        scheduled_task(deadline, 0).partial_cmp(&scheduled_task(deadline, 1)),
        Some(CompareOrdering::Greater)
    );
}

/// Verifies cancellation paths for completable scheduled task entries.
pub fn verify_completable_scheduled_task_cancellation_paths() {
    let (handle, slot) = TaskEndpointPair::<usize, ()>::new().into_parts();
    let cancelled = Arc::new(Atomic::new(false));
    let entry = Box::new(CompletableScheduledTask::new(
        || Ok::<usize, ()>(42),
        slot,
        Arc::clone(&cancelled),
    ));
    entry.accept();
    assert!(!entry.is_cancelled());
    let started_task = entry
        .start()
        .expect("pending task entry should start successfully");
    started_task();
    assert_eq!(handle.get().expect("started task should succeed"), 42);
    assert!(!cancelled.load());

    let (handle, slot) = TaskEndpointPair::<usize, ()>::new().into_parts();
    let cancelled = Arc::new(Atomic::new(false));
    let entry = Box::new(CompletableScheduledTask::new(
        || Ok::<usize, ()>(42),
        slot,
        Arc::clone(&cancelled),
    ));
    entry.accept();
    assert!(entry.cancel());
    assert!(cancelled.load());
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));

    let (tracked, slot) =
        TaskEndpointPair::<usize, ()>::new().into_tracked_parts();
    let cancelled = Arc::new(Atomic::new(false));
    let entry = Box::new(CompletableScheduledTask::new(
        || Ok::<usize, ()>(42),
        slot,
        Arc::clone(&cancelled),
    ));

    assert_eq!(tracked.cancel(), CancelResult::Cancelled);
    assert!(entry.start().is_none());
    assert!(cancelled.load());
    assert!(matches!(tracked.get(), Err(TaskExecutionError::Cancelled)));

    let (tracked, slot) =
        TaskEndpointPair::<usize, ()>::new().into_tracked_parts();
    let cancelled = Arc::new(Atomic::new(false));
    let entry = Box::new(CompletableScheduledTask::new(
        || Ok::<usize, ()>(42),
        slot,
        Arc::clone(&cancelled),
    ));

    assert_eq!(tracked.cancel(), CancelResult::Cancelled);
    assert!(!entry.cancel());
    assert!(!cancelled.load());
    assert!(matches!(tracked.get(), Err(TaskExecutionError::Cancelled)));
}

/// Verifies lifecycle and counter-invariant paths on shared scheduler state.
pub fn verify_single_thread_scheduled_executor_service_inner_paths() {
    let inner = SingleThreadScheduledExecutorServiceInner::new();
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        inner.finish_queued_cancellation();
    }));
    assert!(result.is_err(), "queued counter underflow should panic");
    assert_eq!(inner.queued_count(), 0);

    let inner = SingleThreadScheduledExecutorServiceInner::new();
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        inner.start_task();
    }));
    assert!(result.is_err(), "task start counter underflow should panic");
    assert_eq!(inner.queued_count(), 0);
    assert_eq!(inner.running_count(), 0);

    let inner = SingleThreadScheduledExecutorServiceInner::new();
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        inner.finish_running_task();
    }));
    assert!(result.is_err(), "running counter underflow should panic");
    assert_eq!(inner.running_count(), 0);

    let inner = SingleThreadScheduledExecutorServiceInner::default();

    assert!(!inner.is_not_running());

    inner.shutdown();

    assert!(inner.is_not_running());

    let inner = SingleThreadScheduledExecutorServiceInner::new();
    {
        let mut state = inner.state.lock();
        state.tasks.push(ScheduledTask::new(
            Instant::now(),
            0,
            Box::new(CancelRejectedEntry),
        ));
        inner.add_queued_task();
    }

    let report = inner.stop();

    assert_eq!(report.queued, 1);
    assert_eq!(report.cancelled, 0);
    assert_eq!(report.running, 0);
}
