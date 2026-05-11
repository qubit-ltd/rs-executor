/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Admission gate used to order task lifecycle hooks.

use qubit_lock::ArcMonitor;

/// Gate that blocks a spawned worker until submission acceptance is published.
///
/// Thread-spawning executors can create the worker before they can safely emit
/// `on_accepted`. This gate keeps the worker from starting task execution until
/// the submitting thread has crossed the accepted lifecycle boundary.
#[derive(Clone, Default)]
pub(crate) struct TaskAdmissionGate {
    /// Monitor state set to `true` once the accepted event has been published.
    ready: ArcMonitor<bool>,
}

impl TaskAdmissionGate {
    /// Creates a closed admission gate.
    ///
    /// # Returns
    ///
    /// A gate that blocks waiters until [`Self::open`] is called.
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            ready: ArcMonitor::new(false),
        }
    }

    /// Blocks until the submitting thread opens the gate.
    ///
    /// This method may block indefinitely if the spawning thread never opens the
    /// gate. Executor implementations call it only in workers whose spawn call
    /// has already succeeded.
    #[inline]
    pub(crate) fn wait(&self) {
        self.ready.wait_until(|ready| *ready, |_ready| {});
    }

    /// Opens the gate and wakes the waiting worker.
    ///
    /// This method should be called after the task has emitted its accepted hook.
    #[inline]
    pub(crate) fn open(&self) {
        self.ready.write(|ready| {
            *ready = true;
        });
        self.ready.notify_one();
    }
}
