// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use qubit_function::Callable;

use super::Executor;
use super::thread_spawn_config::ThreadSpawnConfig;
use crate::TrackedTask;
use crate::hook::TaskHook;
use crate::hook::notify_rejected_optional;
use crate::service::SubmissionError;
use crate::task::spi::TaskEndpointPair;
use crate::task::task_admission_gate::TaskAdmissionGate;

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
/// The returned [`TrackedTask`] is created immediately. Dropping the handle
/// does not cancel the helper thread; use [`TrackedTask::cancel`] before the
/// helper thread starts the task when pre-start cancellation is needed.
#[derive(Clone)]
pub struct DelayExecutor {
    /// Duration to sleep before each submitted task starts.
    delay: Duration,
    /// Hook notified about accepted task lifecycle events.
    hook: Option<Arc<dyn TaskHook>>,
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
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            hook: None,
            stack_size: None,
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
        self.hook = Some(hook);
        self
    }

    /// Returns a copy of this executor using the supplied helper thread stack
    /// size.
    ///
    /// # Parameters
    ///
    /// * `stack_size` - Stack size in bytes for each helper thread.
    ///
    /// # Returns
    ///
    /// This executor configured with `stack_size`.
    #[inline]
    pub fn with_stack_size(mut self, stack_size: usize) -> Self {
        self.stack_size = Some(stack_size);
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
    fn spawn_worker(&self, worker: Worker) -> Result<(), SubmissionError> {
        ThreadSpawnConfig::new(self.stack_size).spawn(worker)
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
    fn call<C, R, E>(
        &self,
        task: C,
    ) -> Result<TrackedTask<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let (handle, slot) =
            TaskEndpointPair::with_optional_hook(self.hook.clone())
                .into_tracked_parts();
        let delay = self.delay;
        let gate = TaskAdmissionGate::new(self.hook.is_some());
        let worker_gate = gate.clone();
        let hook = self.hook.clone();
        self.spawn_worker(Box::new(move || {
            worker_gate.wait();
            if !delay.is_zero() {
                thread::sleep(delay);
            }
            slot.run(task);
        }))
        .inspect_err(|error| notify_rejected_optional(hook.as_ref(), error))?;
        handle.accept();
        gate.open();
        Ok(handle)
    }
}
