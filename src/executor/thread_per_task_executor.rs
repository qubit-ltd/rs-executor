/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::sync::Arc;

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
use super::ThreadPerTaskExecutorBuilder;
use super::thread_spawn_config::ThreadSpawnConfig;

/// Executes each task on a dedicated OS thread.
///
/// This executor does not manage lifecycle or maintain a queue. Each accepted
/// task receives a [`TrackedTask`] that can be used to wait for the result.
///
/// # Semantics
///
/// * **One task, one thread** — each [`Executor::call`] or [`Executor::execute`]
///   spawns a new OS thread. There is no pool and no submission queue.
/// * **Blocking or async wait** — [`TrackedTask::get`] blocks the calling thread,
///   while awaiting the handle uses a waker and does not block the polling
///   thread.
/// * **Completion probe** — [`TrackedTask::is_done`] reads an atomic flag set
///   after the worker publishes the result; it does not retrieve the value
///   (you still need [`TrackedTask::get`] for that).
///
/// # Examples
///
/// ```rust
/// use std::io;
///
/// use qubit_executor::executor::{
///     Executor,
///     ThreadPerTaskExecutor,
/// };
///
/// let executor = ThreadPerTaskExecutor::new();
/// let handle = executor
///     .call(|| Ok::<i32, io::Error>(40 + 2))
///     .expect("worker thread should spawn");
///
/// // Blocks the current thread until the spawned thread completes.
/// let value = handle.get().expect("task should succeed");
/// assert_eq!(value, 42);
/// ```
#[derive(Clone)]
pub struct ThreadPerTaskExecutor {
    /// Optional stack size for each spawned worker thread.
    pub(crate) stack_size: Option<usize>,
    /// Hook notified about accepted task lifecycle events.
    pub(crate) hook: Arc<dyn TaskHook>,
}

impl ThreadPerTaskExecutor {
    /// Creates an executor using the platform default worker stack size.
    ///
    /// # Returns
    ///
    /// A thread-per-task executor with default worker thread configuration.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a builder for configuring this executor.
    ///
    /// # Returns
    ///
    /// A builder initialized with default worker thread options.
    #[inline]
    pub fn builder() -> ThreadPerTaskExecutorBuilder {
        ThreadPerTaskExecutorBuilder::new()
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

    /// Spawns one worker thread.
    ///
    /// # Parameters
    ///
    /// * `worker` - Closure to run on the new OS thread.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the worker was spawned.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::WorkerSpawnFailed`] if the operating system
    /// refuses to create the worker thread.
    fn spawn_worker(&self, worker: impl FnOnce() + Send + 'static) -> Result<(), SubmissionError> {
        ThreadSpawnConfig::new(self.stack_size).spawn(worker)
    }
}

impl Default for ThreadPerTaskExecutor {
    /// Creates an executor using the platform default worker stack size and no-op hook.
    #[inline]
    fn default() -> Self {
        Self {
            stack_size: None,
            hook: Arc::new(NoopTaskHook),
        }
    }
}

impl Executor for ThreadPerTaskExecutor {
    /// Spawns one OS thread for the callable and returns a handle to its result.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to run on a dedicated OS thread.
    ///
    /// # Returns
    ///
    /// A [`TrackedTask`] that can block or await the spawned task's final
    /// result.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::WorkerSpawnFailed`] if the worker thread
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
        if let Err(error) = self.spawn_worker(move || {
            slot.run(task);
        }) {
            self.hook.on_rejected(&error);
            return Err(error);
        }
        Ok(handle)
    }
}
