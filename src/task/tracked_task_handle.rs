// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::cancel_result::CancelResult;
use super::task_result_handle::TaskResultHandle;
use super::task_status::TaskStatus;

/// Adds lifecycle observation and pre-start cancellation to a task result
/// handle.
///
/// Executor services use this interface for associated tracked-handle types.
/// The status is an observation of the task state and may race with publishing
/// the final result to the handle. Cancellation is cooperative and only wins
/// while the task is pending; implementations may report that cancellation is
/// unsupported.
///
/// # Type Parameters
///
/// * `R` - Type of the task's successful result.
/// * `E` - Type of the task's callable error.
///
/// # Examples
///
/// ```rust
/// use std::time::Duration;
///
/// use qubit_executor::CancelResult;
/// use qubit_executor::ScheduledExecutorService;
/// use qubit_executor::SingleThreadScheduledExecutorService;
/// use qubit_executor::TaskStatus;
/// use qubit_executor::task::spi::TrackedTaskHandle;
///
/// let service = SingleThreadScheduledExecutorService::new("example-scheduler")
///     .expect("scheduler thread should start");
/// let task = service
///     .schedule_callable(Duration::from_secs(60), || Ok::<_, ()>(42))
///     .expect("scheduled task should be accepted");
/// assert_eq!(TrackedTaskHandle::status(&task), TaskStatus::Pending);
/// assert_eq!(TrackedTaskHandle::cancel(&task), CancelResult::Cancelled);
/// service.shutdown();
/// service.wait_termination();
/// ```
pub trait TrackedTaskHandle<R, E>: TaskResultHandle<R, E> {
    /// Observes the task's current lifecycle state.
    ///
    /// The returned value is a snapshot; the task may transition immediately
    /// afterward. A terminal status may also become visible before the final
    /// result is available from the result handle.
    ///
    /// # Returns
    ///
    /// The current pending, running, or terminal task status.
    fn status(&self) -> TaskStatus;

    /// Attempts to cancel the task while it is still pending.
    ///
    /// Cancellation succeeds only if this call wins the transition from the
    /// pending state. It does not interrupt a task that has started, and an
    /// implementation that cannot cancel active tasks may report
    /// [`CancelResult::Unsupported`].
    ///
    /// # Returns
    ///
    /// A precise cancellation result describing whether cancellation won.
    fn cancel(&self) -> CancelResult;
}
