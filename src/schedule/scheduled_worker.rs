// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::sync::Arc;
use std::time::Instant;

use super::scheduled_task_entry::StartedScheduledTask;
use super::scheduler_core::SchedulerCore;

/// Worker loop entry point for single-thread scheduled executor services.
pub(crate) struct ScheduledWorker;

impl ScheduledWorker {
    /// Runs the scheduled executor service loop.
    pub(crate) fn run(core: Arc<SchedulerCore>) {
        loop {
            let Some(task) = next_ready_task(&core) else {
                return;
            };
            let _ = catch_unwind(AssertUnwindSafe(task));
            finish_task(&core);
        }
    }
}

/// Obtains the next due task without retaining the monitor during task work.
fn next_ready_task(core: &SchedulerCore) -> Option<StartedScheduledTask> {
    let mut state = core.state.lock();
    loop {
        if state.can_terminate() {
            state.terminated = true;
            state.notify_all();
            return None;
        }
        if state.stop_draining {
            state.wait();
            continue;
        }
        let Some(first) = state.tasks.first() else {
            state.wait();
            continue;
        };
        let deadline = *first.order();
        let now = Instant::now();
        if deadline > now {
            let _ = state
                .wait_for(deadline.saturating_duration_since(now))
                .expect("scheduler worker waiter should remain registered");
            continue;
        }
        let entry = state
            .tasks
            .pop_first()
            .expect("observed first scheduled entry must remain present");
        state.worker_active = true;
        drop(state);
        if let Some(task) = entry.into_value().start() {
            return Some(task);
        }
        finish_task(core);
        state = core.state.lock();
    }
}

/// Records that the worker released its active entry.
fn finish_task(core: &SchedulerCore) {
    core.state.with_write_notify_all(|state| {
        state.worker_active = false;
        if state.can_terminate() {
            state.terminated = true;
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Instant;

    use super::super::scheduled_task_entry::ScheduledTaskEntry;
    use super::super::scheduled_task_entry::StartedScheduledTask;
    use super::super::scheduler_core::SchedulerCore;
    use super::ScheduledWorker;
    use crate::hook::TaskId;

    struct MaybeSkippedEntry {
        skip: bool,
        panic_on_run: bool,
        ran: Arc<AtomicUsize>,
    }

    impl ScheduledTaskEntry for MaybeSkippedEntry {
        fn accept(&self) {}

        fn start(self: Box<Self>) -> Option<StartedScheduledTask> {
            if self.skip {
                return None;
            }
            let ran = Arc::clone(&self.ran);
            let panic_on_run = self.panic_on_run;
            Some(Box::new(move || {
                if panic_on_run {
                    panic!("scheduled worker test panic");
                }
                ran.fetch_add(1, Ordering::AcqRel);
            }))
        }

        fn cancel(self: Box<Self>) -> bool {
            false
        }
    }

    #[test]
    fn test_cancelled_due_entry_does_not_exit_worker() {
        let core = Arc::new(SchedulerCore::new());
        let ran = Arc::new(AtomicUsize::new(0));
        let now = Instant::now();
        core.schedule(
            TaskId::new(1),
            now,
            Box::new(MaybeSkippedEntry {
                skip: true,
                panic_on_run: false,
                ran: Arc::clone(&ran),
            }),
        )
        .expect("first entry should schedule");
        core.schedule(
            TaskId::new(2),
            now,
            Box::new(MaybeSkippedEntry {
                skip: false,
                panic_on_run: false,
                ran: Arc::clone(&ran),
            }),
        )
        .expect("second entry should schedule");
        core.shutdown();

        ScheduledWorker::run(Arc::clone(&core));

        assert_eq!(ran.load(Ordering::Acquire), 1);
        assert!(!core.state.with_read(|state| state.worker_active));
        assert!(core.is_terminated());
    }

    #[test]
    fn test_panicking_started_entry_does_not_kill_worker() {
        let core = Arc::new(SchedulerCore::new());
        let ran = Arc::new(AtomicUsize::new(0));
        let now = Instant::now();
        core.schedule(
            TaskId::new(3),
            now,
            Box::new(MaybeSkippedEntry {
                skip: false,
                panic_on_run: true,
                ran: Arc::clone(&ran),
            }),
        )
        .expect("panicking entry should schedule");
        core.schedule(
            TaskId::new(4),
            now,
            Box::new(MaybeSkippedEntry {
                skip: false,
                panic_on_run: false,
                ran: Arc::clone(&ran),
            }),
        )
        .expect("following entry should schedule");
        core.shutdown();

        ScheduledWorker::run(Arc::clone(&core));

        assert_eq!(ran.load(Ordering::Acquire), 1);
        assert!(!core.state.with_read(|state| state.worker_active));
        assert!(core.is_terminated());
    }
}
