/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
use std::future::Future;

use qubit_function::{
    Callable,
    Runnable,
};

use super::{
    RejectedExecution,
    ShutdownReport,
};

/// Managed task service with submission and lifecycle control.
///
/// `ExecutorService` is intentionally separate from
/// [`Executor`](crate::executor::Executor). An executor describes an
/// execution strategy; an executor service accepts tasks into a managed service
/// that may queue, schedule, assign workers, and track lifecycle.
///
/// `submit` and `submit_callable` return `Result` values whose outer `Ok`
/// means only that the service accepted the task. It does **not** mean the task
/// has started or succeeded. The task's final result is observed through the
/// returned handle.
///
/// # Author
///
/// Haixing Hu
pub trait ExecutorService: Send + Sync {
    /// Handle returned for an accepted task.
    type Handle<R, E>
    where
        R: Send + 'static,
        E: Send + 'static;

    /// Future returned when waiting for service termination.
    type Termination<'a>: Future<Output = ()> + Send + 'a
    where
        Self: 'a;

    /// Submits a runnable task to this service.
    ///
    /// # Parameters
    ///
    /// * `task` - A fallible background action with no business return value.
    ///
    /// # Returns
    ///
    /// `Ok(handle)` if the service accepts the task. This only reports
    /// acceptance; it does not report task start or task success. Returns
    /// `Err(RejectedExecution)` if the service refuses the task before
    /// accepting it.
    ///
    /// # Errors
    ///
    /// Returns [`RejectedExecution`] when the service refuses the task before
    /// accepting it.
    #[inline]
    fn submit<T, E>(&self, task: T) -> Result<Self::Handle<(), E>, RejectedExecution>
    where
        T: Runnable<E> + Send + 'static,
        E: Send + 'static,
    {
        let mut task = task;
        self.submit_callable(move || task.run())
    }

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
    /// observed through the returned handle. Returns `Err(RejectedExecution)` if
    /// the service refuses the task before accepting it.
    ///
    /// # Errors
    ///
    /// Returns [`RejectedExecution`] when the service refuses the task before
    /// accepting it.
    fn submit_callable<C, R, E>(&self, task: C) -> Result<Self::Handle<R, E>, RejectedExecution>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static;

    /// Initiates an orderly shutdown.
    ///
    /// After shutdown starts, new tasks are rejected. Already accepted tasks are
    /// allowed to complete unless the concrete service documents stronger
    /// cancellation behavior.
    fn shutdown(&self);

    /// Attempts to stop accepting and running tasks immediately.
    ///
    /// # Returns
    ///
    /// A count-based shutdown report describing the state observed at the time
    /// of the request.
    fn shutdown_now(&self) -> ShutdownReport;

    /// Returns whether shutdown has been requested.
    ///
    /// # Returns
    ///
    /// `true` if this service is no longer accepting new tasks.
    fn is_shutdown(&self) -> bool;

    /// Returns whether the service has terminated.
    ///
    /// # Returns
    ///
    /// `true` only after shutdown has been requested and all accepted tasks have
    /// completed or been cancelled.
    fn is_terminated(&self) -> bool;

    /// Waits until the service has terminated.
    ///
    /// # Returns
    ///
    /// A future that completes after shutdown has been requested and no accepted
    /// tasks remain active.
    fn await_termination(&self) -> Self::Termination<'_>;
}
