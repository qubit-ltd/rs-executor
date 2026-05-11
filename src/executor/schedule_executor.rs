/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::{
    sync::Arc,
    thread,
    time::Instant,
};

use qubit_function::Callable;

use crate::{
    TrackedTask,
    hook::{
        NoopTaskHook,
        TaskHook,
    },
    service::SubmissionError,
    task::spi::TaskEndpointPair,
};

use super::Executor;

/// Executor that starts each task at a specified monotonic instant.
///
/// `ScheduleExecutor` creates the returned [`TrackedTask`] immediately, starts
/// a helper OS thread, waits on that helper thread until the configured
/// [`Instant`], and then runs the task on the helper thread. If the configured
/// instant is not in the future, the helper thread runs the task immediately.
#[derive(Clone)]
pub struct ScheduleExecutor {
    /// Monotonic instant at which each submitted task starts.
    instant: Instant,
    /// Hook notified about accepted task lifecycle events.
    hook: Arc<dyn TaskHook>,
}

impl ScheduleExecutor {
    /// Creates an executor that starts tasks at the supplied monotonic instant.
    ///
    /// # Parameters
    ///
    /// * `instant` - Monotonic instant at which each submitted task starts.
    ///
    /// # Returns
    ///
    /// A schedule executor using `instant`.
    #[inline]
    pub fn at(instant: Instant) -> Self {
        Self {
            instant,
            hook: Arc::new(NoopTaskHook),
        }
    }

    /// Returns a copy of this executor using the supplied task hook.
    ///
    /// # Parameters
    ///
    /// * `hook` - Hook notified about accepted task lifecycle events.
    ///
    /// # Returns
    ///
    /// This executor configured with `hook`.
    #[inline]
    pub fn with_hook(mut self, hook: Arc<dyn TaskHook>) -> Self {
        self.hook = hook;
        self
    }

    /// Returns the configured scheduled instant.
    ///
    /// # Returns
    ///
    /// The monotonic instant at which each task starts.
    #[inline]
    pub const fn instant(&self) -> Instant {
        self.instant
    }
}

impl Executor for ScheduleExecutor {
    /// Starts a helper thread that waits until the scheduled instant and then
    /// runs the callable.
    fn call<C, R, E>(&self, task: C) -> Result<TrackedTask<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let (handle, slot) =
            TaskEndpointPair::with_hook(Arc::clone(&self.hook)).into_tracked_parts();
        self.hook.on_accepted(handle.task_id());
        let instant = self.instant;
        if let Err(error) = thread::Builder::new()
            .spawn(move || {
                let now = Instant::now();
                if instant > now {
                    thread::sleep(instant.duration_since(now));
                }
                slot.run(task);
            })
            .map(drop)
            .map_err(SubmissionError::worker_spawn_failed)
        {
            self.hook.on_rejected(&error);
            return Err(error);
        }
        Ok(handle)
    }
}
