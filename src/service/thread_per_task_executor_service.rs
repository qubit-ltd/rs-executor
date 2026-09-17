// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;
use std::time::Duration;

use qubit_function::Callable;
use qubit_function::Runnable;

use super::ExecutorService;
use super::ExecutorServiceLifecycle;
use super::StopReport;
use super::SubmissionError;
use super::ThreadPerTaskExecutorServiceBuilder;
use crate::TaskHandle;
use crate::TrackedTask;
use crate::executor::thread_spawn_config::ThreadSpawnConfig;
use crate::hook::TaskHook;
use crate::hook::notify_rejected;
use crate::hook::notify_rejected_optional;
use crate::task::spi::TaskEndpointPair;
use crate::task::spi::TaskSlot;
use crate::task::task_admission_gate::TaskAdmissionGate;
#[path = "internal/mod.rs"]
mod internal;
use self::internal::ActiveTaskGuard;
use self::internal::TaskAdmissionHandle;
use self::internal::ThreadPerTaskExecutorServiceState;
/// Type-erased worker executed by a service-owned OS thread.
type Worker = Box<dyn FnOnce() + Send + 'static>;

/// Managed service that runs every accepted task on a dedicated OS thread.
///
/// The service has no queue: accepted tasks start immediately on their own
/// thread. Shutdown prevents later submissions but cannot forcefully stop
/// running OS threads.
///
/// # Examples
///
/// ```rust
/// use qubit_executor::{ExecutorService, ThreadPerTaskExecutorService};
///
/// let service = ThreadPerTaskExecutorService::new();
/// let task = service
///     .submit_callable(|| Ok::<_, ()>(42))
///     .expect("task should be accepted");
/// assert_eq!(task.get().expect("task should succeed"), 42);
/// service.shutdown();
/// service.wait_termination();
/// ```
#[derive(Clone)]
pub struct ThreadPerTaskExecutorService {
    /// Shared service state used by all clones of this service.
    state: Arc<ThreadPerTaskExecutorServiceState>,
    /// Optional stack size for each spawned worker thread.
    stack_size: Option<usize>,
    /// Hook notified about accepted task lifecycle events.
    pub(crate) hook: Option<Arc<dyn TaskHook>>,
}

impl Default for ThreadPerTaskExecutorService {
    /// Creates a service with default worker options and no hook.
    #[inline]
    fn default() -> Self {
        Self {
            state: Arc::default(),
            stack_size: None,
            hook: None,
        }
    }
}

impl ThreadPerTaskExecutorService {
    /// Creates a new service instance.
    ///
    /// # Returns
    ///
    /// A service that accepts tasks until shutdown is requested.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a service with the supplied worker stack size configuration.
    ///
    /// # Parameters
    ///
    /// * `stack_size` - Optional stack size in bytes for spawned workers.
    ///
    /// # Returns
    ///
    /// A service using the supplied worker stack size configuration.
    #[inline]
    pub(crate) fn from_stack_size(stack_size: Option<usize>) -> Self {
        Self {
            state: Arc::default(),
            stack_size,
            hook: None,
        }
    }

    /// Creates a builder for configuring this service.
    ///
    /// # Returns
    ///
    /// A builder initialized with default worker thread options.
    #[inline]
    pub fn builder() -> ThreadPerTaskExecutorServiceBuilder {
        ThreadPerTaskExecutorServiceBuilder::new()
    }

    /// Spawns one accepted worker thread.
    ///
    /// # Parameters
    ///
    /// * `worker` - Closure to run on the worker OS thread.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the worker was spawned.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::WorkerSpawnFailed`] if the operating system
    /// refuses to create the worker thread. Accepted task accounting is handled
    /// by the active-task guard captured by `worker`.
    fn spawn_worker_after_accept(&self, worker: Worker) -> Result<(), SubmissionError> {
        ThreadSpawnConfig::new(self.stack_size).spawn(worker)
    }

    /// Notifies the configured hook about a rejected submission.
    ///
    /// # Parameters
    ///
    /// * `error` - Submission failure reported to the caller.
    #[inline]
    fn notify_rejected(&self, error: &SubmissionError) {
        if let Some(hook) = &self.hook {
            notify_rejected(hook.as_ref(), error);
        }
    }

    /// Accepts service work, starts a worker thread, and returns the chosen
    /// handle.
    ///
    /// # Parameters
    ///
    /// * `split_pair` - Splits the task endpoint pair into the desired handle
    ///   and slot.
    /// * `run_slot` - Worker body that consumes the runner-side task slot.
    ///
    /// # Returns
    ///
    /// The accepted task handle produced by `split_pair`.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::Shutdown`] if the service is not running, or
    /// [`SubmissionError::WorkerSpawnFailed`] if the worker thread cannot be
    /// created.
    fn submit_with_slot<R, E, H, S, F>(&self, split_pair: S, run_slot: F) -> Result<H, SubmissionError>
    where
        R: Send + 'static,
        E: Send + 'static,
        H: TaskAdmissionHandle,
        S: FnOnce(TaskEndpointPair<R, E>) -> (H, TaskSlot<R, E>),
        F: FnOnce(TaskSlot<R, E>) + Send + 'static,
    {
        if let Err(error) = self.state.accept_task() {
            self.notify_rejected(&error);
            return Err(error);
        }

        let pair = TaskEndpointPair::with_optional_hook(self.hook.clone());
        let (handle, slot) = split_pair(pair);
        let guard = ActiveTaskGuard::new(Arc::clone(&self.state));
        let gate = TaskAdmissionGate::new(self.hook.is_some());
        let worker_gate = gate.clone();
        let hook = self.hook.clone();
        if let Err(error) = self.spawn_worker_after_accept(Box::new(move || {
            worker_gate.wait();
            let _guard = guard;
            run_slot(slot);
        })) {
            notify_rejected_optional(hook.as_ref(), &error);
            return Err(error);
        }
        handle.mark_accepted();
        gate.open();
        Ok(handle)
    }
}

impl ExecutorService for ThreadPerTaskExecutorService {
    type ResultHandle<R, E>
        = TaskHandle<R, E>
    where
        R: Send + 'static,
        E: Send + 'static;

    type TrackedHandle<R, E>
        = TrackedTask<R, E>
    where
        R: Send + 'static,
        E: Send + 'static;

    /// Accepts a runnable and starts it on a dedicated OS thread.
    ///
    /// # Parameters
    ///
    /// * `task` - Runnable to execute on a new OS thread.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the runnable was accepted.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::Shutdown`] if shutdown has already been
    /// requested before the task is accepted.
    fn submit<T, E>(&self, task: T) -> Result<(), SubmissionError>
    where
        T: Runnable<E> + Send + 'static,
        E: Send + 'static,
    {
        let handle = self.submit_with_slot(
            |pair| pair.into_parts(),
            move |slot| {
                let mut task = task;
                slot.run(move || task.run());
            },
        )?;
        drop(handle);
        Ok(())
    }

    /// Accepts a callable and starts it on a dedicated OS thread.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to execute on a new OS thread.
    ///
    /// # Returns
    ///
    /// A [`TaskHandle`] for the accepted task.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::Shutdown`] if shutdown has already been
    /// requested before the task is accepted.
    fn submit_callable<C, R, E>(&self, task: C) -> Result<Self::ResultHandle<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        self.submit_with_slot(
            |pair| pair.into_parts(),
            move |slot| {
                slot.run(task);
            },
        )
    }

    /// Accepts a callable and starts it with a tracked handle.
    fn submit_tracked_callable<C, R, E>(&self, task: C) -> Result<Self::TrackedHandle<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        self.submit_with_slot(
            |pair| pair.into_tracked_parts(),
            move |slot| {
                slot.run(task);
            },
        )
    }

    /// Stops accepting new tasks.
    ///
    /// Already accepted threads are allowed to finish.
    fn shutdown(&self) {
        self.state.shutdown();
    }

    /// Stops accepting new tasks and reports currently running work.
    ///
    /// Running OS threads cannot be forcefully stopped by this service.
    ///
    /// # Returns
    ///
    /// A report with zero queued tasks, the observed active thread count, and
    /// zero cancelled tasks.
    fn stop(&self) -> StopReport {
        let running = self.state.stop();
        StopReport::new(0, running, 0)
    }

    /// Returns the current lifecycle state.
    #[inline]
    fn lifecycle(&self) -> ExecutorServiceLifecycle {
        self.state.lifecycle()
    }

    /// Blocks until all accepted tasks complete after shutdown or stop.
    ///
    /// This method blocks the current thread on a condition variable. Calling
    /// it while the service is still running will wait until another thread
    /// calls [`Self::shutdown`] or [`Self::stop`] and all accepted OS-thread
    /// tasks have completed.
    #[inline]
    fn wait_termination(&self) {
        self.state.wait_for_termination();
    }

    /// Waits at most `timeout` for all accepted threads to complete.
    #[inline]
    fn wait_termination_timeout(&self, timeout: Duration) -> bool {
        self.state.wait_for_termination_timeout(timeout)
    }
}
