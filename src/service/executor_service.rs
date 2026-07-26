// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_function::{Callable, Runnable};

use crate::task::spi::{TaskResultHandle, TrackedTaskHandle};

use super::{ExecutorServiceLifecycle, StopReport, SubmissionError};

/// Managed task service with submission and lifecycle control.
///
/// `ExecutorService` is intentionally separate from
/// [`Executor`](crate::Executor). An executor describes an
/// execution strategy; an executor service accepts tasks into a managed service
/// that may queue, schedule, assign workers, and track lifecycle.
///
/// `submit` and `submit_callable` return `Result` values whose outer `Ok`
/// means only that the service accepted the task. It does **not** mean the task
/// has started or succeeded. `submit` is fire-and-forget; callable and tracked
/// variants return handles for observing the final task result.
///
/// ## Lifecycle
///
/// A service starts in [`ExecutorServiceLifecycle::Running`]. While running,
/// submissions may be accepted. Calling [`shutdown`](Self::shutdown) starts an
/// orderly shutdown and moves the service toward
/// [`ExecutorServiceLifecycle::ShuttingDown`]: later submissions are rejected,
/// while work accepted before shutdown is allowed to finish normally. Calling
/// [`stop`](Self::stop) starts an abrupt stop and moves the service toward
/// [`ExecutorServiceLifecycle::Stopping`]: later submissions are rejected and
/// the implementation attempts to cancel or abort accepted work that can still
/// be stopped.
///
/// `shutdown` and `stop` are both terminal admission decisions; neither allows
/// the service to become running again. The difference is how accepted work is
/// treated. `shutdown` preserves accepted work, including queued or scheduled
/// work, unless a concrete service documents a stronger policy. `stop` is a
/// best-effort interruption request for queued, scheduled, unstarted, or
/// runtime-abortable work. Services built with
/// [`TaskSlot`](crate::task::spi::TaskSlot) should publish
/// [`TaskExecutionError::Cancelled`](crate::TaskExecutionError::Cancelled) for
/// accepted work that is intentionally removed before it starts, typically by
/// calling
/// [`TaskSlot::cancel_unstarted`](crate::task::spi::TaskSlot::cancel_unstarted).
/// Work already running in ordinary Rust code, blocking calls, or OS threads
/// may not be forcibly interrupted, so termination can still wait for that work
/// to return.
///
/// A service reaches [`ExecutorServiceLifecycle::Terminated`] after shutdown or
/// stop has been requested and no accepted work remains active. Accepted work
/// may have completed normally, failed, panicked, been cancelled, or been
/// dropped by its runner endpoint, or been aborted according to the concrete
/// service's capabilities.
///
/// ## Resource cleanup
///
/// Dropping an executor-service handle is not a portable resource-release
/// protocol. Concrete services may request shutdown from `Drop`, but `Drop`
/// should not be assumed to block until worker threads, helper threads, runtime
/// tasks, queues, or other service-owned resources have fully exited. Blocking
/// in `Drop` would make ordinary handle destruction unexpectedly wait for
/// arbitrary user code, blocking calls, or OS-thread tasks that cannot be
/// interrupted.
///
/// Code that needs deterministic cleanup must request termination explicitly
/// and then wait for it:
///
/// 1. Call [`shutdown`](Self::shutdown) to drain accepted work, or
///    [`stop`](Self::stop) to request best-effort cancellation or abort of work
///    that has not become non-interruptible.
/// 2. Call [`wait_termination`](Self::wait_termination) to block until the
///    service reports that no accepted work remains active.
/// 3. Drop the service handle and any task handles after the wait returns.
///
/// If a service owns OS threads or blocking tasks, already-running task bodies
/// can keep external resources such as file descriptors, sockets, locks, or
/// reference-counted objects alive until those task bodies return. Services
/// that need stronger cleanup behavior should expose an explicit close/join API
/// rather than relying on destructor side effects.
pub trait ExecutorService: Send + Sync {
    /// Result handle returned for an accepted callable task.
    type ResultHandle<R, E>: TaskResultHandle<R, E>
    where
        R: Send + 'static,
        E: Send + 'static;

    /// Tracked handle returned for accepted tasks that expose status.
    type TrackedHandle<R, E>: TrackedTaskHandle<R, E>
    where
        R: Send + 'static,
        E: Send + 'static;

    /// Submits a runnable task to this service.
    ///
    /// # Parameters
    ///
    /// * `task` - A fallible background action with no business return value.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the service accepts the task. This only reports acceptance;
    /// it does not report task start or task success. Returns
    /// `Err(SubmissionError)` if the service refuses the task before
    /// accepting it.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError`] when the service refuses the task before
    /// accepting it.
    fn submit<T, E>(&self, task: T) -> Result<(), SubmissionError>
    where
        T: Runnable<E> + Send + 'static,
        E: Send + 'static;

    /// Submits a callable task to this service.
    ///
    /// # Parameters
    ///
    /// * `task` - A fallible computation whose success value should be captured
    ///   in the returned handle.
    ///
    /// # Returns
    ///
    /// `Ok(handle)` if the service accepts the task. This only reports
    /// acceptance; task success, task failure, panic, or cancellation must be
    /// observed through the returned handle. Returns `Err(SubmissionError)` if
    /// the service refuses the task before accepting it.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError`] when the service refuses the task before
    /// accepting it.
    fn submit_callable<C, R, E>(
        &self,
        task: C,
    ) -> Result<Self::ResultHandle<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static;

    /// Submits a runnable task and returns a tracked handle.
    ///
    /// # Parameters
    ///
    /// * `task` - A fallible background action with no business return value.
    ///
    /// # Returns
    ///
    /// `Ok(handle)` if the service accepts the task. The handle exposes status,
    /// pre-start cancellation, and final unit result retrieval.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError`] when the service refuses the task before
    /// accepting it.
    #[inline]
    fn submit_tracked<T, E>(&self, task: T) -> Result<Self::TrackedHandle<(), E>, SubmissionError>
    where
        T: Runnable<E> + Send + 'static,
        E: Send + 'static,
    {
        let mut task = task;
        self.submit_tracked_callable(move || task.run())
    }

    /// Submits a callable task and returns a tracked handle.
    ///
    /// # Parameters
    ///
    /// * `task` - A fallible computation whose success value should be captured
    ///   in the returned handle.
    ///
    /// # Returns
    ///
    /// `Ok(handle)` if the service accepts the task. The handle exposes status,
    /// pre-start cancellation, and final result retrieval.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError`] when the service refuses the task before
    /// accepting it.
    fn submit_tracked_callable<C, R, E>(
        &self,
        task: C,
    ) -> Result<Self::TrackedHandle<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static;

    /// Initiates an orderly shutdown.
    ///
    /// After shutdown starts, the service rejects new submissions and enters
    /// the [`ExecutorServiceLifecycle::ShuttingDown`] path. Already accepted
    /// work is allowed to complete normally, including work that is queued,
    /// scheduled, or running, unless the concrete service documents a stronger
    /// cancellation policy.
    ///
    /// This method is an admission gate change, not a wait operation. Use
    /// [`wait_termination`](Self::wait_termination) to block until all accepted
    /// work has completed or the service has otherwise terminated.
    fn shutdown(&self);

    /// Attempts to stop accepting new tasks and stop accepted work immediately.
    ///
    /// After stop starts, the service rejects new submissions and enters the
    /// [`ExecutorServiceLifecycle::Stopping`] path. The implementation should
    /// cancel queued, scheduled, or unstarted work where possible, and abort
    /// runtime-managed work where its runtime provides an abort mechanism.
    ///
    /// `stop` is best effort. It cannot promise to interrupt arbitrary Rust
    /// code, blocking calls, or already-running OS-thread work. Such work may
    /// continue until it returns, and service termination waits for any
    /// non-interruptible accepted work that remains active.
    ///
    /// # Returns
    ///
    /// A count-based stop report describing queued, running, and cancelled work
    /// observed while handling the request.
    fn stop(&self) -> StopReport;

    /// Returns the current lifecycle state.
    ///
    /// # Returns
    ///
    /// The lifecycle state currently observed by this service.
    fn lifecycle(&self) -> ExecutorServiceLifecycle;

    /// Returns whether the service accepts new tasks.
    ///
    /// # Returns
    ///
    /// `true` only while the lifecycle is
    /// [`ExecutorServiceLifecycle::Running`].
    #[inline]
    fn is_running(&self) -> bool {
        self.lifecycle() == ExecutorServiceLifecycle::Running
    }

    /// Returns whether graceful shutdown is in progress.
    ///
    /// # Returns
    ///
    /// `true` only while the lifecycle is
    /// [`ExecutorServiceLifecycle::ShuttingDown`].
    #[inline]
    fn is_shutting_down(&self) -> bool {
        self.lifecycle() == ExecutorServiceLifecycle::ShuttingDown
    }

    /// Returns whether abrupt stop is in progress.
    ///
    /// # Returns
    ///
    /// `true` only while the lifecycle is
    /// [`ExecutorServiceLifecycle::Stopping`].
    #[inline]
    fn is_stopping(&self) -> bool {
        self.lifecycle() == ExecutorServiceLifecycle::Stopping
    }

    /// Returns whether this service is not running.
    ///
    /// # Returns
    ///
    /// `true` once the service has started graceful shutdown, abrupt stop, or
    /// has already terminated.
    #[inline]
    fn is_not_running(&self) -> bool {
        self.lifecycle() != ExecutorServiceLifecycle::Running
    }

    /// Returns whether the service has terminated.
    ///
    /// # Returns
    ///
    /// `true` only after shutdown or stop has been requested and all accepted
    /// tasks have completed or been cancelled.
    #[inline]
    fn is_terminated(&self) -> bool {
        self.lifecycle() == ExecutorServiceLifecycle::Terminated
    }

    /// Blocks the current thread until the service has terminated.
    ///
    /// This method is a synchronous, blocking wait. It returns only after
    /// [`shutdown`](Self::shutdown) or [`stop`](Self::stop) has been requested
    /// and no accepted tasks remain active. If it is called while the service
    /// is still [`ExecutorServiceLifecycle::Running`] and no other thread
    /// requests shutdown or stop, it may block forever.
    ///
    /// This method is the portable way to wait for service-owned resources to
    /// quiesce after an explicit shutdown or stop request. Dropping a service
    /// handle is not a substitute for calling this method when deterministic
    /// cleanup matters.
    ///
    /// Implementations must not present this method as an asynchronous or
    /// non-blocking operation.
    fn wait_termination(&self);
}
