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
    service::RejectedExecution,
    task::{
        TaskCompletionPair,
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
    /// Optional stack size for each helper thread.
    stack_size: Option<usize>,
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
        Self {
            delay,
            stack_size: None,
        }
    }

    /// Creates a delayed executor with an explicit helper thread stack size.
    ///
    /// # Parameters
    ///
    /// * `delay` - Duration to wait before running each task.
    /// * `stack_size` - Stack size in bytes for each spawned helper thread.
    ///
    /// # Returns
    ///
    /// A delay executor using the supplied delay and helper stack size.
    #[inline]
    pub const fn with_stack_size(delay: Duration, stack_size: usize) -> Self {
        Self {
            delay,
            stack_size: Some(stack_size),
        }
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

    fn spawn_worker(&self, worker: Worker) -> Result<(), RejectedExecution> {
        let mut builder = thread::Builder::new();
        if let Some(stack_size) = self.stack_size {
            builder = builder.stack_size(stack_size);
        }
        builder
            .spawn(worker)
            .map(drop)
            .map_err(RejectedExecution::worker_spawn_failed)
    }
}

impl Executor for DelayExecutor {
    type Execution<R, E>
        = Result<TrackedTask<R, E>, RejectedExecution>
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
    /// Returns [`RejectedExecution::WorkerSpawnFailed`] if the helper thread
    /// cannot be created.
    fn call<C, R, E>(&self, task: C) -> Self::Execution<R, E>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let (handle, completion) = TaskCompletionPair::new().into_tracked_parts();
        let delay = self.delay;
        self.spawn_worker(Box::new(move || {
            if !delay.is_zero() {
                thread::sleep(delay);
            }
            TaskRunner::new(task).run(completion);
        }))?;
        Ok(handle)
    }
}
