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

use super::internal::WorkerActiveGuard;
use super::scheduled_task_entry::StartedScheduledTask;
use super::scheduler_core::SchedulerCore;

/// Worker loop entry point for single-thread scheduled executor services.
pub(crate) struct ScheduledWorker;

impl ScheduledWorker {
    /// Runs the scheduled executor service loop.
    ///
    /// Ordinary panics from starting or running one entry are contained for
    /// that entry; its active marker is restored before the worker continues.
    /// Aborting panics cannot be recovered.
    pub(crate) fn run(core: Arc<SchedulerCore>) {
        loop {
            let Some(task) = next_ready_task(&core) else {
                return;
            };
            let _active_guard = WorkerActiveGuard::new(&core);
            let _ = catch_unwind(AssertUnwindSafe(task));
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
        return Some(Box::new(move || {
            if let Some(task) = entry.into_value().start() {
                task();
            }
        }));
    }
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

    struct PanicOnStartEntry {}

    impl ScheduledTaskEntry for PanicOnStartEntry {
        fn accept(&self) {}

        fn start(self: Box<Self>) -> Option<StartedScheduledTask> {
            panic!("scheduled worker start panic");
        }

        fn cancel(self: Box<Self>) -> bool {
            false
        }
    }

    struct SkipWithPanickingDropEntry;

    impl ScheduledTaskEntry for SkipWithPanickingDropEntry {
        fn accept(&self) {}

        fn start(self: Box<Self>) -> Option<StartedScheduledTask> {
            None
        }

        fn cancel(self: Box<Self>) -> bool {
            false
        }
    }

    impl Drop for SkipWithPanickingDropEntry {
        fn drop(&mut self) {
            panic!("scheduled worker skipped-entry drop panic");
        }
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

    #[test]
    fn test_panicking_start_does_not_kill_worker() {
        let core = Arc::new(SchedulerCore::new());
        let ran = Arc::new(AtomicUsize::new(0));
        let now = Instant::now();
        core.schedule(TaskId::new(3), now, Box::new(PanicOnStartEntry {}))
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
        .expect("follow-up entry should schedule");
        core.shutdown();

        ScheduledWorker::run(Arc::clone(&core));

        assert_eq!(ran.load(Ordering::Acquire), 1);
        assert!(core.is_terminated());
    }

    #[test]
    fn test_panicking_skipped_entry_drop_does_not_kill_worker() {
        let core = Arc::new(SchedulerCore::new());
        let ran = Arc::new(AtomicUsize::new(0));
        let now = Instant::now();
        core.schedule(TaskId::new(5), now, Box::new(SkipWithPanickingDropEntry))
            .expect("panicking drop entry should schedule");
        core.schedule(
            TaskId::new(6),
            now,
            Box::new(MaybeSkippedEntry {
                skip: false,
                panic_on_run: false,
                ran: Arc::clone(&ran),
            }),
        )
        .expect("follow-up entry should schedule");
        core.shutdown();

        ScheduledWorker::run(Arc::clone(&core));

        assert_eq!(ran.load(Ordering::Acquire), 1);
        assert!(!core.state.with_read(|state| state.worker_active));
        assert!(core.is_terminated());
    }
}
