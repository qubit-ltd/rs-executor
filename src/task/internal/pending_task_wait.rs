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
            self.changed
                .wait_for(&mut cancelled, remaining.min(Duration::from_secs(3600)));
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
        let worker_wait = Arc::clone(&wait);
        let worker = thread::spawn(move || {
            entered_tx.send(()).expect("test should observe waiter start");
            worker_wait.wait_for(Duration::from_secs(3600))
        });

        entered_rx.recv().expect("waiter should start");
        wait.cancel();
        assert!(!worker.join().expect("waiter should return"));
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
        let worker_wait = Arc::clone(&wait);
        let worker = thread::spawn(move || {
            entered_tx.send(()).expect("test should observe waiter start");
            worker_wait.wait_for(Duration::from_secs(3600))
        });

        entered_rx.recv().expect("waiter should start");
        wait.changed.notify_one();
        wait.cancel();
        assert!(!worker.join().expect("waiter should return"));
    }

    #[test]
    fn test_max_duration_wait_can_be_cancelled() {
        let wait = Arc::new(PendingTaskWait::new());
        let (entered_tx, entered_rx) = mpsc::channel();
        let worker_wait = Arc::clone(&wait);
        let worker = thread::spawn(move || {
            entered_tx.send(()).expect("test should observe waiter start");
            worker_wait.wait_for(Duration::MAX)
        });

        entered_rx.recv().expect("waiter should start");
        wait.cancel();
        assert!(!worker.join().expect("waiter should return"));
    }
}
