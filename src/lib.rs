// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! # Qubit Executor
//!
//! Core executor abstractions, task handles, and basic executor
//! implementations.

use std::time::Duration;

use qubit_clock::TimeError;
use qubit_lock::{
    ParkingLotMonitor,
    WaitTimeoutResult,
};

pub mod executor;
pub mod hook;
pub mod schedule;
pub mod service;
pub mod task;

pub use crate::executor::{
    DelayExecutor,
    DirectExecutor,
    Executor,
    ScheduleExecutor,
    ThreadPerTaskExecutor,
    ThreadPerTaskExecutorBuilder,
};
pub use crate::schedule::{
    ScheduledExecutorService,
    ScheduledTaskHandle,
    SingleThreadScheduledExecutorService,
};
pub use crate::service::{
    ExecutorService,
    ExecutorServiceBuilderError,
    ExecutorServiceLifecycle,
    StopReport,
    SubmissionError,
    ThreadPerTaskExecutorService,
    ThreadPerTaskExecutorServiceBuilder,
};
pub use crate::task::{
    CancelResult,
    TaskExecutionError,
    TaskResult,
    TaskStatus,
};
pub use crate::task::{
    TaskHandle,
    TrackedTask,
    TryGet,
};

/// Waits for a monitor predicate with a total call-duration budget.
///
/// The budget starts before attempting to acquire the monitor lock, so lock
/// contention, predicate evaluation, waiting, and lock reacquisition all
/// consume it. This differs from
/// [`ParkingLotMonitor::wait_until_ready_for`], whose condition-wait budget
/// starts only after the monitor lock has been acquired.
///
/// # Parameters
///
/// * `monitor` - Monitor that owns the predicate state.
/// * `timeout` - Maximum duration for the entire call.
/// * `predicate` - Returns `true` when the protected state is ready.
///
/// # Returns
///
/// Returns `Ok(true)` when the predicate is ready and `Ok(false)` when the
/// total deadline expires while the predicate remains false.
///
/// # Errors
///
/// Returns [`TimeError`] when the Timer cannot construct or complete the
/// deadline-bound wait.
///
/// # Panics
///
/// Propagates a panic from `predicate`.
pub fn wait_until_ready_with_total_timeout<T, P>(
    monitor: &ParkingLotMonitor<T>,
    timeout: Duration,
    predicate: P,
) -> Result<bool, TimeError>
where
    P: FnMut(&T) -> bool,
{
    let deadline = monitor.timer().deadline_after(timeout)?;
    let result = monitor.wait_until_ready_with_deadline(deadline, predicate)?;
    Ok(matches!(result, WaitTimeoutResult::Ready(())))
}
