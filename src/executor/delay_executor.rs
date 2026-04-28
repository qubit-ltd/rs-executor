/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
use std::{
    thread,
    time::Duration,
};

use qubit_function::Callable;

use crate::{
    TaskCompletionPair,
    TaskHandle,
    task_runner::run_callable,
};

use super::Executor;

/// Executor that starts each task after a fixed delay.
///
/// `DelayExecutor` models delayed start, not minimum execution duration. The
/// returned [`TaskHandle`] is created immediately. A helper thread sleeps for
/// the configured delay and then runs the task. Dropping the handle does not
/// cancel the helper thread.
///
/// # Author
///
/// Haixing Hu
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
}

impl Executor for DelayExecutor {
    type Execution<R, E>
        = TaskHandle<R, E>
    where
        R: Send + 'static,
        E: std::fmt::Display + Send + 'static;

    /// Starts a helper thread that waits and then runs the callable.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to run after the configured delay.
    ///
    /// # Returns
    ///
    /// A [`TaskHandle`] for the delayed task.
    fn call<C, R, E>(&self, task: C) -> Self::Execution<R, E>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        let (handle, completion) = TaskCompletionPair::new().into_parts();
        let delay = self.delay;
        thread::spawn(move || {
            if !delay.is_zero() {
                thread::sleep(delay);
            }
            completion.start_and_complete(|| run_callable(task));
        });
        handle
    }
}
