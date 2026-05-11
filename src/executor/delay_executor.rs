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
    time::Duration,
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

type Worker = Box<dyn FnOnce() + Send + 'static>;

/// Executor that starts each task after a fixed delay.
///
/// `DelayExecutor` models delayed start, not minimum execution duration. The
/// returned [`TrackedTask`] is created immediately. A helper thread sleeps for
/// the configured delay and then runs the task. Dropping the handle does not
/// cancel the helper thread.
///
#[derive(Clone)]
pub struct DelayExecutor {
    /// Duration to sleep before each submitted task starts.
    delay: Duration,
    /// Hook notified about accepted task lifecycle events.
    hook: Arc<dyn TaskHook>,
}

impl DelayExecutor {
    /// Creates an executor that delays task start by the supplied duration.
    ///
    /// # Parameters
    ///
    /// * `delay` - Duration to wait before running each task.
    ///
    /// # Returns
    ///
    /// A delay executor using the supplied delay.
    #[inline]
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
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

    /// Returns the configured delay.
    ///
    /// # Returns
    ///
    /// The duration waited before each task starts.
    #[inline]
    pub const fn delay(&self) -> Duration {
        self.delay
    }

    /// Spawns one delayed worker thread.
    ///
    /// # Parameters
    ///
    /// * `worker` - Closure to run on the helper OS thread.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the helper was spawned.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::WorkerSpawnFailed`] if the operating system
    /// refuses to create the helper thread.
    fn spawn_worker(worker: Worker) -> Result<(), SubmissionError> {
        thread::Builder::new()
            .spawn(worker)
            .map(drop)
            .map_err(SubmissionError::worker_spawn_failed)
    }
}

impl Executor for DelayExecutor {
    /// Starts a helper thread that waits and then runs the callable.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to run after the configured delay.
    ///
    /// # Returns
    ///
    /// A [`TrackedTask`] for the delayed task.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::WorkerSpawnFailed`] if the helper thread
    /// cannot be created.
    fn call<C, R, E>(&self, task: C) -> Result<TrackedTask<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let (handle, slot) =
            TaskEndpointPair::with_hook(Arc::clone(&self.hook)).into_tracked_parts();
        self.hook.on_accepted(handle.task_id());
        let delay = self.delay;
        if let Err(error) = Self::spawn_worker(Box::new(move || {
            if !delay.is_zero() {
                thread::sleep(delay);
            }
            slot.run(task);
        })) {
            self.hook.on_rejected(&error);
            return Err(error);
        }
        Ok(handle)
    }
}
