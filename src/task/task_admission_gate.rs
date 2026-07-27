// =============================================================================
// qubit-style: allow source-test-pair
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Admission gate used to order task lifecycle hooks.

use qubit_lock::ArcParkingLotMonitor;

/// Gate that optionally blocks a spawned worker until acceptance is published.
///
/// Thread-spawning executors with hooks can create the worker before they can
/// safely emit `on_accepted`. The blocking variant keeps the worker from
/// starting task execution until the submitting thread has crossed the accepted
/// lifecycle boundary. The open variant is used on the no-hook fast path and
/// performs no allocation or synchronization.
#[derive(Clone)]
pub(crate) enum TaskAdmissionGate {
    /// No-op gate used when no hook ordering needs to be enforced.
    Open,
    /// Monitor-backed gate used when hook ordering must be enforced.
    Blocked(ArcParkingLotMonitor<bool>),
}

impl TaskAdmissionGate {
    /// Creates an admission gate for the supplied hook configuration.
    ///
    /// # Parameters
    ///
    /// * `requires_ordering` - Whether worker start must wait for accepted hook
    ///   publication.
    ///
    /// # Returns
    ///
    /// A blocking gate when ordering is required, or a no-op gate otherwise.
    #[inline]
    pub(crate) fn new(requires_ordering: bool) -> Self {
        if requires_ordering {
            Self::Blocked(ArcParkingLotMonitor::new(false))
        } else {
            Self::Open
        }
    }

    /// Blocks until the submitting thread opens this gate.
    ///
    /// The open variant returns immediately. The blocking variant may block
    /// indefinitely if the spawning thread never opens the gate. Executor
    /// implementations call it only in workers whose spawn call has already
    /// succeeded.
    #[inline]
    pub(crate) fn wait(&self) {
        if let Self::Blocked(ready) = self {
            ready.wait_until(|ready| *ready, |_ready| {});
        }
    }

    /// Opens the gate and wakes the waiting worker.
    ///
    /// This method should be called after the task has emitted its accepted
    /// hook. It is a no-op for the open variant.
    #[inline]
    pub(crate) fn open(&self) {
        if let Self::Blocked(ready) = self {
            ready.with_write_notify_one(|ready| {
                *ready = true;
            });
        }
    }
}
