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
    thread,
    time::Duration,
};

use qubit_function::Callable;

use crate::{
    TrackedTask,
    service::SubmissionError,
    task::{
        TaskEndpointPair,
        TaskRunner,
    },
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DelayExecutor {
    /// Duration to sleep before each submitted task starts.
    delay: Duration,
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
    pub const fn new(delay: Duration) -> Self {
        Self { delay }
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
    type Output<R, E>
        = TrackedTask<R, E>
    where
        R: Send + 'static,
        E: Send + 'static;

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
    fn call<C, R, E>(&self, task: C) -> Result<Self::Output<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let (handle, completion) = TaskEndpointPair::new().into_tracked_parts();
        let delay = self.delay;
        Self::spawn_worker(Box::new(move || {
            if !delay.is_zero() {
                thread::sleep(delay);
            }
            TaskRunner::new(task).run(completion);
        }))?;
        Ok(handle)
    }
}
