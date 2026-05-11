/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::thread;

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
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ThreadPerTaskExecutor {
    /// Optional stack size for each spawned worker thread.
    stack_size: Option<usize>,
}

impl ThreadPerTaskExecutor {
    /// Creates an executor using the platform default worker stack size.
    ///
    /// # Returns
    ///
    /// A thread-per-task executor with default worker thread configuration.
    #[inline]
    pub const fn new() -> Self {
        Self { stack_size: None }
    }

    /// Creates an executor with an explicit worker thread stack size.
    ///
    /// # Parameters
    ///
    /// * `stack_size` - Stack size in bytes for each spawned worker thread.
    ///
    /// # Returns
    ///
    /// A thread-per-task executor that applies the supplied stack size to each
    /// worker thread.
    #[inline]
    pub const fn with_stack_size(stack_size: usize) -> Self {
        Self {
            stack_size: Some(stack_size),
        }
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

impl Executor for ThreadPerTaskExecutor {
    type Execution<R, E>
        = Result<TrackedTask<R, E>, RejectedExecution>
    where
        R: Send + 'static,
        E: Send + 'static;

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
    /// Returns [`RejectedExecution::WorkerSpawnFailed`] if the worker thread
    /// cannot be created.
    fn call<C, R, E>(&self, task: C) -> Self::Execution<R, E>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let (handle, completion) = TaskCompletionPair::new().into_tracked_parts();
        self.spawn_worker(Box::new(move || {
            TaskRunner::new(task).run(completion);
        }))?;
        Ok(handle)
    }
}
