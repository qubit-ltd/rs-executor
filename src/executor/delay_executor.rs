/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::{sync::Arc, thread, time::Duration};

use qubit_function::Callable;

use crate::{
    TrackedTask,
    hook::{TaskHook, notify_rejected},
    service::SubmissionError,
    task::spi::TaskEndpointPair,
};

use super::{Executor, task_admission_gate::TaskAdmissionGate};

type Worker = Box<dyn FnOnce() + Send + 'static>;

/// Executor that starts each task after a fixed delay.
///
/// `DelayExecutor` is a small non-blocking submission strategy for cases where
/// one task should run later while the caller must return immediately. It
/// models delayed start, not minimum execution duration.
///
/// Each accepted task gets its own helper OS thread. The helper thread sleeps
/// for the configured delay and then runs the task. This keeps the submitting
/// thread unblocked without requiring a shared timer, queue, runtime, or
/// scheduler worker. It is intentionally not a general-purpose delayed task
/// scheduler and does not coalesce timers across tasks.
///
/// The returned [`TrackedTask`] is created immediately. Dropping the handle does
/// not cancel the helper thread; use [`TrackedTask::cancel`] before the helper
/// thread starts the task when pre-start cancellation is needed.
///
#[derive(Clone)]
pub struct DelayExecutor {
    /// Duration to sleep before each submitted task starts.
    delay: Duration,
    /// Hook notified about accepted task lifecycle events.
    hook: Option<Arc<dyn TaskHook>>,
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
        Self { delay, hook: None }
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
        self.hook = Some(hook);
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
    /// This method returns after the helper thread has been spawned; it does
    /// not wait for the configured delay or for task completion.
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
            TaskEndpointPair::with_optional_hook(self.hook.clone()).into_tracked_parts();
        let delay = self.delay;
        if self.hook.is_some() {
            let gate = TaskAdmissionGate::new();
            let worker_gate = gate.clone();
            Self::spawn_worker(Box::new(move || {
                worker_gate.wait();
                if !delay.is_zero() {
                    thread::sleep(delay);
                }
                slot.run(task);
            }))
            .inspect_err(|error| {
                if let Some(hook) = &self.hook {
                    notify_rejected(hook.as_ref(), error);
                }
            })?;
            handle.accept();
            gate.open();
            return Ok(handle);
        }
        Self::spawn_worker(Box::new(move || {
            if !delay.is_zero() {
                thread::sleep(delay);
            }
            slot.run(task);
        }))?;
        handle.accept();
        Ok(handle)
    }
}
