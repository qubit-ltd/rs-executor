// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::time::Duration;
use std::time::Instant;

use parking_lot::Condvar;
use parking_lot::Mutex;
use parking_lot::MutexGuard;

/// A private, cancellation-aware delay used by per-task delayed workers.
///
/// Cancellation is persistent, so it is observed even if it happens before
/// the worker starts waiting. The wait does not own task lifecycle state.
pub(crate) struct PendingTaskWait {
    /// Whether a pending task's wait has been cancelled.
    cancelled: Mutex<bool>,
    /// Wakes a worker after cancellation or a spurious notification.
    changed: Condvar,
}

impl PendingTaskWait {
    /// Creates a wait that has not been cancelled.
    pub(crate) fn new() -> Self {
        Self {
            cancelled: Mutex::new(false),
            changed: Condvar::new(),
        }
    }

    /// Marks this wait cancelled and wakes a waiting worker.
    pub(crate) fn cancel(&self) {
        *self.cancelled.lock() = true;
        self.changed.notify_one();
    }

    /// Waits for `delay`, returning `false` if cancellation is observed.
    ///
    /// Long waits are split into one-hour intervals so very large durations
    /// do not need to be converted to an overflowing absolute deadline.
    pub(crate) fn wait_for(&self, delay: Duration) -> bool {
        self.wait_for_with_waiter(delay, |changed, cancelled, interval| {
            changed.wait_for(cancelled, interval);
        })
    }

    /// Runs the delay loop with a replaceable condition-variable wait operation.
    ///
    /// The waiter is called while `cancelled` is locked and must release and
    /// reacquire that lock as part of waiting. The method returns `false` when
    /// cancellation is observed and `true` when the delay has elapsed.
    fn wait_for_with_waiter<F>(&self, delay: Duration, mut waiter: F) -> bool
    where
        F: for<'a, 'b> FnMut(&Condvar, &'a mut MutexGuard<'b, bool>, Duration),
    {
        let start = Instant::now();
        let mut cancelled = self.cancelled.lock();
        loop {
            if *cancelled {
                return false;
            }
            let remaining = delay.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return true;
            }
            waiter(
                &self.changed,
                &mut cancelled,
                remaining.min(Duration::from_secs(3600)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use super::PendingTaskWait;

    fn cancel_with_timeout(wait: &Arc<PendingTaskWait>) -> bool {
        let (cancelled_tx, cancelled_rx) = mpsc::channel();
        let worker_wait = Arc::clone(wait);
        let canceler = thread::spawn(move || {
            worker_wait.cancel();
            let _ = cancelled_tx.send(());
        });
        let completed = cancelled_rx.recv_timeout(Duration::from_secs(5)).is_ok();
        if completed {
            let _ = canceler.join();
        } else {
            drop(canceler);
        }
        completed
    }

    fn clean_up_worker(
        wait: &PendingTaskWait,
        done_rx: &mpsc::Receiver<bool>,
        worker: thread::JoinHandle<()>,
        primary_result: Result<bool, mpsc::RecvTimeoutError>,
    ) -> (
        Result<bool, mpsc::RecvTimeoutError>,
        Option<thread::Result<()>>,
        bool,
    ) {
        if let Some(mut cancelled) = wait.cancelled.try_lock_for(Duration::from_secs(5)) {
            *cancelled = true;
            drop(cancelled);
        }
        wait.changed.notify_all();
        let cleanup_result = if primary_result.is_ok() {
            true
        } else {
            done_rx.recv_timeout(Duration::from_secs(5)).is_ok()
        };
        let joined = if primary_result.is_ok() || cleanup_result {
            Some(worker.join())
        } else {
            drop(worker);
            None
        };
        (primary_result, joined, cleanup_result)
    }

    #[test]
    fn test_cancel_before_wait_returns_cancelled() {
        let wait = PendingTaskWait::new();
        wait.cancel();
        assert!(!wait.wait_for(Duration::from_secs(1)));
    }

    #[test]
    fn test_cancel_wakes_waiting_thread() {
        let wait = Arc::new(PendingTaskWait::new());
        let (entered_tx, entered_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker_wait = Arc::clone(&wait);
        let worker = thread::spawn(move || {
            let result = worker_wait.wait_for_with_waiter(
                Duration::from_secs(3600),
                |changed, cancelled, interval| {
                    let _ = entered_tx.send(());
                    changed.wait_for(cancelled, interval);
                },
            );
            let _ = done_tx.send(result);
        });

        let stage_seen = entered_rx.recv_timeout(Duration::from_secs(5)).is_ok();
        let lock_released = stage_seen
            && wait
                .cancelled
                .try_lock_for(Duration::from_secs(5))
                .is_some();
        let cancel_completed = cancel_with_timeout(&wait);
        let primary_result = done_rx.recv_timeout(Duration::from_secs(5));
        let (result, joined, cleanup_completed) =
            clean_up_worker(&wait, &done_rx, worker, primary_result);

        assert!(
            stage_seen,
            "worker should reach a real condition-variable wait"
        );
        assert!(
            lock_released,
            "condition-variable wait should release the cancellation lock"
        );
        assert!(
            cancel_completed,
            "cancellation should finish within five seconds"
        );
        assert!(
            matches!(result, Ok(false)),
            "cancellation should finish the wait within five seconds"
        );
        assert!(
            cleanup_completed,
            "worker should complete during bounded cleanup"
        );
        assert!(
            matches!(joined, Some(Ok(()))),
            "worker should exit without panic"
        );
    }

    #[test]
    fn test_zero_delay_completes_immediately() {
        let wait = PendingTaskWait::new();
        assert!(wait.wait_for(Duration::ZERO));
    }

    #[test]
    fn test_notification_without_cancellation_keeps_waiting() {
        let wait = Arc::new(PendingTaskWait::new());
        let (entered_tx, entered_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker_wait = Arc::clone(&wait);
        let worker = thread::spawn(move || {
            let result = worker_wait.wait_for_with_waiter(
                Duration::from_secs(3600),
                |changed, cancelled, interval| {
                    let _ = entered_tx.send(());
                    changed.wait_for(cancelled, interval);
                },
            );
            let _ = done_tx.send(result);
        });

        let first_stage_seen = entered_rx.recv_timeout(Duration::from_secs(5)).is_ok();
        let first_lock_released = first_stage_seen
            && wait
                .cancelled
                .try_lock_for(Duration::from_secs(5))
                .is_some();
        let second_stage_seen = if first_lock_released {
            let cancelled = wait.cancelled.try_lock_for(Duration::from_secs(5));
            match cancelled {
                Some(guard) => {
                    while entered_rx.try_recv().is_ok() {}
                    let no_result_before_notification = done_rx.try_recv().is_err();
                    wait.changed.notify_one();
                    drop(guard);
                    let stage_seen = entered_rx.recv_timeout(Duration::from_secs(5)).is_ok();
                    let lock_released = stage_seen
                        && wait
                            .cancelled
                            .try_lock_for(Duration::from_secs(5))
                            .is_some();
                    let no_result_after_notification = done_rx.try_recv().is_err();
                    no_result_before_notification && lock_released && no_result_after_notification
                }
                None => false,
            }
        } else {
            false
        };
        let cancel_completed = cancel_with_timeout(&wait);
        let primary_result = done_rx.recv_timeout(Duration::from_secs(5));
        let (result, joined, cleanup_completed) =
            clean_up_worker(&wait, &done_rx, worker, primary_result);

        assert!(
            first_stage_seen,
            "worker should reach its initial real wait"
        );
        assert!(
            first_lock_released,
            "initial wait should release the cancellation lock"
        );
        assert!(
            second_stage_seen,
            "ordinary notification should lead to another real wait before completion"
        );
        assert!(
            cancel_completed,
            "cancellation should finish within five seconds"
        );
        assert!(
            matches!(result, Ok(false)),
            "cancellation should finish the wait within five seconds"
        );
        assert!(
            cleanup_completed,
            "worker should complete during bounded cleanup"
        );
        assert!(
            matches!(joined, Some(Ok(()))),
            "worker should exit without panic"
        );
    }

    #[test]
    fn test_max_duration_wait_can_be_cancelled() {
        let wait = Arc::new(PendingTaskWait::new());
        let (entered_tx, entered_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker_wait = Arc::clone(&wait);
        let worker = thread::spawn(move || {
            let result =
                worker_wait.wait_for_with_waiter(Duration::MAX, |changed, cancelled, interval| {
                    let _ = entered_tx.send(());
                    changed.wait_for(cancelled, interval);
                });
            let _ = done_tx.send(result);
        });

        let stage_seen = entered_rx.recv_timeout(Duration::from_secs(5)).is_ok();
        let lock_released = stage_seen
            && wait
                .cancelled
                .try_lock_for(Duration::from_secs(5))
                .is_some();
        let cancel_completed = cancel_with_timeout(&wait);
        let primary_result = done_rx.recv_timeout(Duration::from_secs(5));
        let (result, joined, cleanup_completed) =
            clean_up_worker(&wait, &done_rx, worker, primary_result);

        assert!(
            stage_seen,
            "worker should reach a real condition-variable wait"
        );
        assert!(
            lock_released,
            "condition-variable wait should release the cancellation lock"
        );
        assert!(
            cancel_completed,
            "cancellation should finish within five seconds"
        );
        assert!(
            matches!(result, Ok(false)),
            "cancellation should finish the maximum-duration wait"
        );
        assert!(
            cleanup_completed,
            "worker should complete during bounded cleanup"
        );
        assert!(
            matches!(joined, Some(Ok(()))),
            "worker should exit without panic"
        );
    }
}
