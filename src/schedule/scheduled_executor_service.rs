// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::time::{Duration, Instant};

use qubit_function::{Callable, Runnable};

use crate::service::{ExecutorService, SubmissionError};

/// Managed executor service with delayed and instant-based submission support.
///
/// `ScheduledExecutorService` extends [`ExecutorService`] with per-task timing.
/// A normal `submit` call is an immediate submission; `schedule` and
/// `schedule_at` accept work now but delay the task start until the requested
/// time. Successful submission only means the service accepted the scheduled
/// task. Task success, failure, panic, or cancellation is observed through the
/// returned tracked handle.
pub trait ScheduledExecutorService: ExecutorService {
    /// Schedules a runnable task to start after the supplied delay.
    ///
    /// # Parameters
    ///
    /// * `delay` - Minimum delay before the task becomes runnable.
    /// * `task` - Runnable to execute after the delay elapses.
    ///
    /// # Returns
    ///
    /// A tracked handle for observing or cancelling the scheduled task.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError`] when the service refuses the task before
    /// accepting it.
    #[inline]
    fn schedule<T, E>(
        &self,
        delay: Duration,
        task: T,
    ) -> Result<Self::TrackedHandle<(), E>, SubmissionError>
    where
        T: Runnable<E> + Send + 'static,
        E: Send + 'static,
    {
        let mut task = task;
        self.schedule_callable(delay, move || task.run())
    }

    /// Schedules a callable task to start after the supplied delay.
    ///
    /// # Parameters
    ///
    /// * `delay` - Minimum delay before the task becomes runnable.
    /// * `task` - Callable to execute after the delay elapses.
    ///
    /// # Returns
    ///
    /// A tracked handle for observing or cancelling the scheduled task.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError`] when the service refuses the task before
    /// accepting it.
    #[inline]
    fn schedule_callable<C, R, E>(
        &self,
        delay: Duration,
        task: C,
    ) -> Result<Self::TrackedHandle<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        self.schedule_callable_at(Instant::now() + delay, task)
    }

    /// Schedules a runnable task to start at a monotonic instant.
    ///
    /// # Parameters
    ///
    /// * `instant` - Monotonic instant at which the task becomes runnable.
    /// * `task` - Runnable to execute at or after the instant.
    ///
    /// # Returns
    ///
    /// A tracked handle for observing or cancelling the scheduled task.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError`] when the service refuses the task before
    /// accepting it.
    #[inline]
    fn schedule_at<T, E>(
        &self,
        instant: Instant,
        task: T,
    ) -> Result<Self::TrackedHandle<(), E>, SubmissionError>
    where
        T: Runnable<E> + Send + 'static,
        E: Send + 'static,
    {
        let mut task = task;
        self.schedule_callable_at(instant, move || task.run())
    }

    /// Schedules a callable task to start at a monotonic instant.
    ///
    /// # Parameters
    ///
    /// * `instant` - Monotonic instant at which the task becomes runnable.
    /// * `task` - Callable to execute at or after the instant.
    ///
    /// # Returns
    ///
    /// A tracked handle for observing or cancelling the scheduled task.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError`] when the service refuses the task before
    /// accepting it.
    fn schedule_callable_at<C, R, E>(
        &self,
        instant: Instant,
        task: C,
    ) -> Result<Self::TrackedHandle<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static;
}
