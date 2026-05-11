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
};

use parking_lot::{
    Condvar,
    Mutex,
};
use qubit_function::{
    Callable,
    Runnable,
};

use crate::{
    TaskHandle,
    TrackedTask,
    task::spi::{
        TaskEndpointPair,
        TaskRunner,
    },
};

use super::{
    ExecutorService,
    ExecutorServiceLifecycle,
    StopReport,
    SubmissionError,
    ThreadPerTaskExecutorServiceBuilder,
};

type Worker = Box<dyn FnOnce() + Send + 'static>;

/// Mutable service state protected by the service mutex.
#[derive(Debug, Clone, Copy)]
struct ServiceState {
    /// Current lifecycle state.
    lifecycle: ExecutorServiceLifecycle,
    /// Number of accepted OS-thread tasks that have not completed.
    active_tasks: usize,
}

impl Default for ServiceState {
    /// Creates a running state with no active tasks.
    #[inline]
    fn default() -> Self {
        Self {
            lifecycle: ExecutorServiceLifecycle::Running,
            active_tasks: 0,
        }
    }
}

/// Shared state for [`ThreadPerTaskExecutorService`].
#[derive(Default)]
struct ThreadPerTaskExecutorServiceState {
    /// Lifecycle and active-task counters protected as one state machine.
    state: Mutex<ServiceState>,
    /// Condition variable used to wait for service termination.
    termination: Condvar,
}

/// Guard that records completion for one accepted task when dropped.
struct ActiveTaskGuard {
    /// Shared service state to update when the worker exits.
    state: Arc<ThreadPerTaskExecutorServiceState>,
}

impl ActiveTaskGuard {
    /// Creates a guard for one accepted active task.
    ///
    /// # Parameters
    ///
    /// * `state` - Shared service state whose active count should be decremented.
    ///
    /// # Returns
    ///
    /// A guard that finishes the accepted task on drop.
    #[inline]
    fn new(state: Arc<ThreadPerTaskExecutorServiceState>) -> Self {
        Self { state }
    }
}

impl Drop for ActiveTaskGuard {
    /// Records task completion when the worker closure exits.
    #[inline]
    fn drop(&mut self) {
        self.state.finish_task();
    }
}

impl ThreadPerTaskExecutorServiceState {
    /// Returns the currently stored lifecycle state.
    ///
    /// # Returns
    ///
    /// The lifecycle stored in the service state.
    #[inline]
    fn lifecycle(&self) -> ExecutorServiceLifecycle {
        self.state.lock().lifecycle
    }

    /// Attempts to accept one task and increments the active task count.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the service is running and accepted the task.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::Shutdown`] if the service is not running.
    #[inline]
    fn accept_task(&self) -> Result<(), SubmissionError> {
        let mut state = self.state.lock();
        if state.lifecycle != ExecutorServiceLifecycle::Running {
            return Err(SubmissionError::Shutdown);
        }
        state.active_tasks += 1;
        Ok(())
    }

    /// Records one task completion and wakes termination waiters if appropriate.
    #[inline]
    fn finish_task(&self) {
        let mut state = self.state.lock();
        if state.active_tasks > 0 {
            state.active_tasks -= 1;
        }
        Self::terminate_if_ready(&mut state, &self.termination);
    }

    /// Blocks the current thread until the service is terminated.
    fn wait_for_termination(&self) {
        let mut state = self.state.lock();
        while state.lifecycle != ExecutorServiceLifecycle::Terminated {
            self.termination.wait(&mut state);
        }
    }

    /// Requests graceful shutdown.
    #[inline]
    fn shutdown(&self) {
        let mut state = self.state.lock();
        if state.lifecycle == ExecutorServiceLifecycle::Running {
            state.lifecycle = ExecutorServiceLifecycle::ShuttingDown;
        }
        Self::terminate_if_ready(&mut state, &self.termination);
    }

    /// Requests abrupt stop and returns the observed active work count.
    ///
    /// # Returns
    ///
    /// The number of active tasks observed while stopping.
    #[inline]
    fn stop(&self) -> usize {
        let mut state = self.state.lock();
        if state.lifecycle != ExecutorServiceLifecycle::Terminated {
            state.lifecycle = ExecutorServiceLifecycle::Stopping;
        }
        let running = state.active_tasks;
        Self::terminate_if_ready(&mut state, &self.termination);
        running
    }

    /// Marks the service terminated when it is non-running and idle.
    #[inline]
    fn terminate_if_ready(state: &mut ServiceState, termination: &Condvar) {
        if state.lifecycle != ExecutorServiceLifecycle::Running && state.active_tasks == 0 {
            state.lifecycle = ExecutorServiceLifecycle::Terminated;
            termination.notify_all();
        }
    }
}

/// Managed service that runs every accepted task on a dedicated OS thread.
///
/// The service has no queue: accepted tasks start immediately on their own
/// thread. Shutdown prevents later submissions but cannot forcefully stop
/// running OS threads.
#[derive(Default, Clone)]
pub struct ThreadPerTaskExecutorService {
    /// Shared service state used by all clones of this service.
    state: Arc<ThreadPerTaskExecutorServiceState>,
    /// Optional stack size for each spawned worker thread.
    stack_size: Option<usize>,
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
        }
    }

    /// Creates a builder for configuring this service.
    ///
    /// # Returns
    ///
    /// A builder initialized with default worker thread options.
    #[inline]
    pub const fn builder() -> ThreadPerTaskExecutorServiceBuilder {
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
        let mut builder = thread::Builder::new();
        if let Some(stack_size) = self.stack_size {
            builder = builder.stack_size(stack_size);
        }
        builder
            .spawn(worker)
            .map(drop)
            .map_err(SubmissionError::worker_spawn_failed)
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
        self.state.accept_task()?;

        let guard = ActiveTaskGuard::new(Arc::clone(&self.state));
        self.spawn_worker_after_accept(Box::new(move || {
            let _guard = guard;
            let mut task = task;
            TaskRunner::new(move || task.run()).run_detached::<(), E>();
        }))
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
        self.state.accept_task()?;

        let (handle, completion) = TaskEndpointPair::new().into_parts();
        let guard = ActiveTaskGuard::new(Arc::clone(&self.state));
        self.spawn_worker_after_accept(Box::new(move || {
            let _guard = guard;
            TaskRunner::new(task).run(completion);
        }))?;
        Ok(handle)
    }

    /// Accepts a callable and starts it with a tracked handle.
    fn submit_tracked_callable<C, R, E>(
        &self,
        task: C,
    ) -> Result<Self::TrackedHandle<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        self.state.accept_task()?;

        let (handle, completion) = TaskEndpointPair::new().into_tracked_parts();
        let guard = ActiveTaskGuard::new(Arc::clone(&self.state));
        self.spawn_worker_after_accept(Box::new(move || {
            let _guard = guard;
            TaskRunner::new(task).run(completion);
        }))?;
        Ok(handle)
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
}
