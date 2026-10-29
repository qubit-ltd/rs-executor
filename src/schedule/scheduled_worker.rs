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
use std::time::Duration;
use std::time::Instant;

use qubit_lock::ParkingLotMonitorGuard;

use super::internal::WorkerActiveGuard;
use super::scheduled_task_entry::StartedScheduledTask;
use super::scheduler_core::SchedulerCore;
use super::scheduler_state::SchedulerState;

/// Worker loop entry point for single-thread scheduled executor services.
pub(crate) struct ScheduledWorker;

impl ScheduledWorker {
    /// Runs the scheduled executor service loop.
    ///
    /// Ordinary panics from starting or running one entry are contained for
    /// that entry; its active marker is restored before the worker continues.
    /// Aborting panics cannot be recovered.
    pub(crate) fn run(core: Arc<SchedulerCore>) {
        Self::run_with_waiter(core, |state, delay| {
            state.wait_for(delay).map(|_| ()).map_err(|error| {
                log::error!("scheduled worker wait failed: {error}");
            })
        });
    }

    /// Runs the worker loop with a custom wait operation for the next deadline.
    ///
    /// The waiter is called while the scheduler monitor guard is held. A
    /// blocking wait must release that lock while waiting and reacquire it
    /// before returning. After a successful return, the worker checks the
    /// scheduler conditions again; a wake alone does not make a task ready.
    /// If the waiter returns `Err`, or its panic is caught by the worker's
    /// unwind handler, the worker requests immediate stop through
    /// [`SchedulerCore::stop`]. Termination is still published only when
    /// `SchedulerState::can_terminate()` is true, so an existing stop drain
    /// remains responsible for its own cleanup and is not bypassed. This stop
    /// request does not guarantee that all task work ends immediately.
    fn run_with_waiter<F>(core: Arc<SchedulerCore>, mut waiter: F)
    where
        F: for<'a, 'b> FnMut(&'a mut ParkingLotMonitorGuard<'b, SchedulerState>, Duration) -> Result<(), ()>,
    {
        let result = catch_unwind(AssertUnwindSafe(|| {
            loop {
                let task = match next_ready_task(&core, &mut waiter)? {
                    Some(task) => task,
                    None => return Ok(()),
                };
                let active_guard = WorkerActiveGuard::new(&core);
                if let Err(payload) = catch_unwind(AssertUnwindSafe(task)) {
                    crate::internal::discard_panic_payload(payload);
                }
                drop(active_guard);
            }
        }));

        match result {
            Ok(Ok(())) => {}
            Ok(Err(())) => stop_after_worker_failure(&core),
            Err(payload) => {
                stop_after_worker_failure(&core);
                crate::internal::discard_panic_payload(payload);
            }
        }
    }
}

/// Stops queued work after the worker loop fails and contains stop panics.
fn stop_after_worker_failure(core: &SchedulerCore) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| core.stop())) {
        crate::internal::discard_panic_payload(payload);
    }
}

/// Obtains the next due task without retaining the monitor during task work.
fn next_ready_task<F>(core: &SchedulerCore, waiter: &mut F) -> Result<Option<StartedScheduledTask>, ()>
where
    F: for<'a, 'b> FnMut(&'a mut ParkingLotMonitorGuard<'b, SchedulerState>, Duration) -> Result<(), ()>,
{
    let mut state = core.state.lock();
    loop {
        if state.can_terminate() {
            state.terminated = true;
            state.notify_all();
            return Ok(None);
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
            waiter(&mut state, deadline.saturating_duration_since(now))?;
            continue;
        }
        let entry = state
            .tasks
            .pop_first()
            .expect("observed first scheduled entry must remain present");
        state.worker_active = true;
        drop(state);
        return Ok(Some(Box::new(move || {
            if let Some(task) = entry.into_value().start() {
                task();
            }
        })));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use std::time::Instant;

    use super::super::completable_scheduled_task::CompletableScheduledTask;
    use super::super::scheduled_task_entry::ScheduledTaskEntry;
    use super::super::scheduled_task_entry::StartedScheduledTask;
    use super::super::scheduler_core::SchedulerCore;
    use super::ScheduledWorker;
    use super::WorkerActiveGuard;
    use super::next_ready_task;
    use crate::CancelResult;
    use crate::TaskExecutionError;
    use crate::TaskStatus;
    use crate::hook::TaskId;
    use crate::task::TryGet;
    use crate::task::spi::TaskEndpointPair;

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

    struct PanicOnCancelEntry;

    struct PanicOnDrop;

    struct BlockingCancelEntry {
        entered: Option<mpsc::Sender<()>>,
        release: Option<mpsc::Receiver<()>>,
    }

    impl ScheduledTaskEntry for BlockingCancelEntry {
        fn accept(&self) {}

        fn start(self: Box<Self>) -> Option<StartedScheduledTask> {
            None
        }

        fn cancel(mut self: Box<Self>) -> bool {
            self.entered
                .take()
                .expect("stop should signal cancellation")
                .send(())
                .expect("test should observe cancellation");
            self.release
                .take()
                .expect("stop should wait for release")
                .recv()
                .expect("test should release cancellation");
            true
        }
    }

    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("scheduled cancellation payload drop panic");
        }
    }

    impl ScheduledTaskEntry for PanicOnCancelEntry {
        fn accept(&self) {}

        fn start(self: Box<Self>) -> Option<StartedScheduledTask> {
            None
        }

        fn cancel(self: Box<Self>) -> bool {
            std::panic::panic_any(PanicOnDrop);
        }
    }

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
    fn test_scheduled_claim_can_start_after_stop() {
        let core = SchedulerCore::new();
        let (tracked, slot) = TaskEndpointPair::<usize, ()>::new().into_tracked_parts();
        core.schedule(
            tracked.task_id(),
            Instant::now(),
            Box::new(CompletableScheduledTask::new(|| Ok::<usize, ()>(42), slot)),
        )
        .expect("queue due task");

        let claimed = next_ready_task(&core, &mut |_, _| -> Result<(), ()> {
            panic!("due task must not wait for a deadline");
        })
        .expect("claim succeeds")
        .expect("due task exists");
        let active = WorkerActiveGuard::new(&core);
        assert_eq!(tracked.status(), TaskStatus::Pending);

        let report = core.stop();

        assert_eq!((report.queued, report.running, report.cancelled), (0, 1, 0));
        assert_eq!(tracked.status(), TaskStatus::Pending);
        claimed();
        drop(active);
        assert_eq!(tracked.get().expect("claimed task may still run"), 42);
        assert!(core.is_terminated());
    }

    #[test]
    fn test_scheduled_claim_can_be_cancelled_before_start() {
        let core = SchedulerCore::new();
        let (tracked, slot) = TaskEndpointPair::<usize, ()>::new().into_tracked_parts();
        let calls = Arc::new(AtomicUsize::new(0));
        let task_calls = Arc::clone(&calls);
        core.schedule(
            tracked.task_id(),
            Instant::now(),
            Box::new(CompletableScheduledTask::new(
                move || {
                    task_calls.fetch_add(1, Ordering::AcqRel);
                    Ok::<usize, ()>(42)
                },
                slot,
            )),
        )
        .expect("queue due task");

        let claimed = next_ready_task(&core, &mut |_, _| -> Result<(), ()> {
            panic!("due task must not wait for a deadline");
        })
        .expect("claim succeeds")
        .expect("due task exists");
        let active = WorkerActiveGuard::new(&core);
        assert_eq!(tracked.cancel(), CancelResult::Cancelled);
        claimed();
        drop(active);

        assert_eq!(calls.load(Ordering::Acquire), 0);
        assert!(matches!(tracked.get(), Err(TaskExecutionError::Cancelled)));
        assert!(!core.state.with_read(|state| state.worker_active));
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
    fn test_panicking_task_payload_does_not_kill_worker() {
        let core = Arc::new(SchedulerCore::new());
        let (panicked, panicked_slot) = TaskEndpointPair::<usize, ()>::new().into_parts();
        let (succeeded, succeeded_slot) = TaskEndpointPair::<usize, ()>::new().into_parts();
        let now = Instant::now();
        core.schedule(
            TaskId::new(7),
            now,
            Box::new(CompletableScheduledTask::new(
                || -> Result<usize, ()> { std::panic::panic_any(PanicOnDrop) },
                panicked_slot,
            )),
        )
        .expect("panicking callable should schedule");
        core.schedule(
            TaskId::new(8),
            now,
            Box::new(CompletableScheduledTask::new(|| Ok::<usize, ()>(42), succeeded_slot)),
        )
        .expect("following callable should schedule");
        core.shutdown();

        ScheduledWorker::run(Arc::clone(&core));

        assert!(matches!(
            panicked.try_get(),
            TryGet::Ready(Err(TaskExecutionError::Panicked))
        ));
        assert!(matches!(succeeded.try_get(), TryGet::Ready(Ok(42))));
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

    #[test]
    fn test_wait_error_cancels_queued_tasks_and_terminates() {
        let core = Arc::new(SchedulerCore::new());
        let (first, first_slot) = TaskEndpointPair::<usize, ()>::new().into_parts();
        let (second, second_slot) = TaskEndpointPair::<usize, ()>::new().into_parts();
        let deadline = Instant::now() + std::time::Duration::from_secs(60);
        core.schedule(
            TaskId::new(100),
            deadline,
            Box::new(CompletableScheduledTask::new(|| Ok::<usize, ()>(1), first_slot)),
        )
        .expect("first entry should schedule");
        core.schedule(
            TaskId::new(101),
            deadline,
            Box::new(CompletableScheduledTask::new(|| Ok::<usize, ()>(2), second_slot)),
        )
        .expect("second entry should schedule");

        let exit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ScheduledWorker::run_with_waiter(Arc::clone(&core), |_, _| Err(()));
        }));
        assert!(exit.is_ok(), "wait error must not escape the worker");
        assert!(matches!(
            first.try_get(),
            TryGet::Ready(Err(TaskExecutionError::Cancelled))
        ));
        assert!(matches!(
            second.try_get(),
            TryGet::Ready(Err(TaskExecutionError::Cancelled))
        ));
        assert_eq!(core.queued_count(), 0);
        assert_eq!(core.running_count(), 0);
        assert!(core.is_terminated());
        assert_eq!(core.lifecycle(), crate::service::ExecutorServiceLifecycle::Terminated);
        let (rejected_handle, rejected_slot) = TaskEndpointPair::<usize, ()>::new().into_parts();
        let rejected = core.schedule(
            TaskId::new(102),
            deadline,
            Box::new(CompletableScheduledTask::new(|| Ok::<usize, ()>(3), rejected_slot)),
        );
        assert!(matches!(rejected, Err(crate::service::SubmissionError::Shutdown)));
        assert!(matches!(
            rejected_handle.try_get(),
            TryGet::Ready(Err(TaskExecutionError::Dropped))
        ));
    }

    #[test]
    fn test_waiter_panic_stops_worker_and_cancels_queued_tasks() {
        let core = Arc::new(SchedulerCore::new());
        let (handle, slot) = TaskEndpointPair::<usize, ()>::new().into_parts();
        core.schedule(
            TaskId::new(103),
            Instant::now() + std::time::Duration::from_secs(60),
            Box::new(CompletableScheduledTask::new(|| Ok::<usize, ()>(1), slot)),
        )
        .expect("entry should schedule");

        ScheduledWorker::run_with_waiter(Arc::clone(&core), |_, _| panic!("injected waiter panic"));

        assert!(matches!(
            handle.try_get(),
            TryGet::Ready(Err(TaskExecutionError::Cancelled))
        ));
        assert_eq!(core.queued_count(), 0);
        assert!(core.is_terminated());
    }

    #[test]
    fn test_wait_error_cancels_following_entries_after_cancel_payload_drop_panics() {
        let core = Arc::new(SchedulerCore::new());
        let (handle, slot) = TaskEndpointPair::<usize, ()>::new().into_parts();
        let deadline = Instant::now() + std::time::Duration::from_secs(60);
        core.schedule(TaskId::new(104), deadline, Box::new(PanicOnCancelEntry))
            .expect("panicking entry should schedule");
        core.schedule(
            TaskId::new(105),
            deadline,
            Box::new(CompletableScheduledTask::new(|| Ok::<usize, ()>(1), slot)),
        )
        .expect("following entry should schedule");

        ScheduledWorker::run_with_waiter(Arc::clone(&core), |_, _| Err(()));

        assert!(matches!(
            handle.try_get(),
            TryGet::Ready(Err(TaskExecutionError::Cancelled))
        ));
        assert_eq!(core.queued_count(), 0);
        assert_eq!(core.running_count(), 0);
        assert!(core.is_terminated());
    }

    #[test]
    fn test_wait_failure_does_not_terminate_during_another_stop_drain() {
        let core = Arc::new(SchedulerCore::new());
        let (cancel_entered_tx, cancel_entered_rx) = mpsc::channel();
        let (cancel_release_tx, cancel_release_rx) = mpsc::channel();
        core.schedule(
            TaskId::new(106),
            Instant::now() + Duration::from_secs(60),
            Box::new(BlockingCancelEntry {
                entered: Some(cancel_entered_tx),
                release: Some(cancel_release_rx),
            }),
        )
        .expect("blocking cancellation entry should schedule");

        let worker_core = Arc::clone(&core);
        let (waiter_entered_tx, waiter_entered_rx) = mpsc::channel();
        let (worker_exited_tx, worker_exited_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            ScheduledWorker::run_with_waiter(worker_core, move |state, delay| {
                waiter_entered_tx.send(()).expect("test should observe waiter entry");
                let _ = state.wait_for(delay);
                Err(())
            });
            let _ = worker_exited_tx.send(());
        });
        let waiter_entered = waiter_entered_rx.recv_timeout(Duration::from_secs(5));
        // The worker holds the monitor while entering the waiter. This read
        // completes only after wait_for releases the monitor to sleep.
        let (probe_tx, probe_rx) = mpsc::channel();
        let probe_core = Arc::clone(&core);
        let probe = thread::spawn(move || {
            probe_core.state.with_read(|_| ());
            let _ = probe_tx.send(());
        });
        let monitor_released = probe_rx.recv_timeout(Duration::from_secs(5));
        if monitor_released.is_ok() {
            probe.join().expect("monitor probe should return");
        } else {
            drop(probe);
        }

        let stop_core = Arc::clone(&core);
        let (stopper_exited_tx, stopper_exited_rx) = mpsc::channel();
        let stopper = thread::spawn(move || {
            let _ = stop_core.stop();
            let _ = stopper_exited_tx.send(());
        });
        let cancel_entered = cancel_entered_rx.recv_timeout(Duration::from_secs(5));
        let worker_exited_while_drain_blocked = worker_exited_rx.recv_timeout(Duration::from_secs(5));
        let worker_exited_during_drain = worker_exited_while_drain_blocked.is_ok();
        let (terminated_probe_tx, terminated_probe_rx) = mpsc::channel();
        let terminated_probe_core = Arc::clone(&core);
        let terminated_probe = thread::spawn(move || {
            let _ = terminated_probe_tx.send(terminated_probe_core.is_terminated());
        });
        let terminated_while_drain_blocked = terminated_probe_rx.recv_timeout(Duration::from_secs(5));
        if terminated_while_drain_blocked.is_ok() {
            terminated_probe.join().expect("termination probe should return");
        } else {
            drop(terminated_probe);
        }

        let _ = cancel_release_tx.send(());
        let worker_exited = if worker_exited_while_drain_blocked.is_ok() {
            worker_exited_while_drain_blocked
        } else {
            worker_exited_rx.recv_timeout(Duration::from_secs(5))
        };
        let worker_joined = if worker_exited.is_ok() {
            Some(worker.join())
        } else {
            drop(worker);
            None
        };
        let stopper_exited = stopper_exited_rx.recv_timeout(Duration::from_secs(5));
        let stopper_joined = if stopper_exited.is_ok() {
            Some(stopper.join())
        } else {
            drop(stopper);
            None
        };

        assert!(waiter_entered.is_ok(), "worker should reach its deadline wait");
        assert!(monitor_released.is_ok(), "waiter should release the scheduler monitor");
        assert!(cancel_entered.is_ok(), "external stop should enter the drain");
        assert!(
            worker_exited_during_drain,
            "failed worker should exit while another stop drains"
        );
        assert_eq!(terminated_while_drain_blocked, Ok(false));
        assert!(
            matches!(worker_joined, Some(Ok(()))),
            "worker should exit during bounded cleanup"
        );
        assert!(
            matches!(stopper_joined, Some(Ok(()))),
            "external stop should finish during bounded cleanup"
        );
        let (final_probe_tx, final_probe_rx) = mpsc::channel();
        let final_probe_core = Arc::clone(&core);
        let final_probe = thread::spawn(move || {
            let _ = final_probe_tx.send(final_probe_core.is_terminated());
        });
        let terminated_after_drain = final_probe_rx.recv_timeout(Duration::from_secs(5));
        if terminated_after_drain.is_ok() {
            final_probe.join().expect("final termination probe should return");
        } else {
            drop(final_probe);
        }
        assert_eq!(terminated_after_drain, Ok(true));
    }

    #[test]
    fn test_scheduled_deadline_earlier_submission_wakes_waiter() {
        let core = Arc::new(SchedulerCore::new());
        let far_deadline = Instant::now() + Duration::from_secs(60);
        let (far_handle, far_slot) = TaskEndpointPair::<usize, ()>::new().into_parts();
        core.schedule(
            TaskId::new(200),
            far_deadline,
            Box::new(CompletableScheduledTask::new(|| Ok::<usize, ()>(1), far_slot)),
        )
        .expect("far-future entry should schedule");

        let (waiter_ready_tx, waiter_ready_rx) = mpsc::channel();
        let (task_ran_tx, task_ran_rx) = mpsc::channel();
        let (worker_exited_tx, worker_exited_rx) = mpsc::channel();
        let (early_handle, early_slot) = TaskEndpointPair::<usize, ()>::new().into_parts();
        let fallback_exit = Arc::new(AtomicBool::new(false));
        let worker_fallback_exit = Arc::clone(&fallback_exit);
        let worker_core = Arc::clone(&core);
        let worker = thread::spawn(move || {
            let mut first_wait = true;
            ScheduledWorker::run_with_waiter(worker_core, move |state, delay| {
                if first_wait {
                    first_wait = false;
                    waiter_ready_tx
                        .send(())
                        .expect("test should observe the first deadline wait");
                }
                let _ = state.wait_for(delay);
                if worker_fallback_exit.load(Ordering::Acquire) {
                    return Err(());
                }
                Ok(())
            });
            let _ = worker_exited_tx.send(());
        });

        let waiter_ready = waiter_ready_rx.recv_timeout(Duration::from_secs(5));
        let scheduled = if waiter_ready.is_ok() {
            core.schedule(
                TaskId::new(201),
                Instant::now(),
                Box::new(CompletableScheduledTask::new(
                    move || {
                        task_ran_tx.send(()).expect("early task observer exists");
                        Ok::<usize, ()>(2)
                    },
                    early_slot,
                )),
            )
        } else {
            Err(crate::service::SubmissionError::Shutdown)
        };
        let task_ran = if scheduled.is_ok() {
            task_ran_rx.recv_timeout(Duration::from_secs(5))
        } else {
            Err(mpsc::RecvTimeoutError::Disconnected)
        };

        let _ = core.stop();
        let mut worker_exited = worker_exited_rx.recv_timeout(Duration::from_millis(250));
        if worker_exited.is_err() {
            fallback_exit.store(true, Ordering::Release);
            core.state.with_write_notify_all(|_| ());
            worker_exited = worker_exited_rx.recv_timeout(Duration::from_secs(5));
        }
        let worker_joined = if worker_exited.is_ok() {
            Some(worker.join())
        } else {
            // Keep the test bounded if even the fallback cannot recover the worker.
            drop(worker);
            None
        };

        assert!(waiter_ready.is_ok(), "worker should enter the far deadline wait");
        assert!(scheduled.is_ok(), "earlier entry should be accepted");
        assert!(
            task_ran.is_ok(),
            "earlier entry should wake and run before the far deadline"
        );
        assert!(worker_exited.is_ok(), "worker should exit after stop");
        assert!(
            matches!(worker_joined, Some(Ok(()))),
            "worker should exit without panicking"
        );
        assert!(matches!(early_handle.try_get(), TryGet::Ready(Ok(2))));
        assert!(matches!(
            far_handle.try_get(),
            TryGet::Ready(Err(TaskExecutionError::Cancelled))
        ));
    }
}
