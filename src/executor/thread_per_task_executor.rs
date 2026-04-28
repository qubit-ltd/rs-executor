/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
use std::thread;

use qubit_function::Callable;

use crate::{
    TaskCompletionPair,
    TaskHandle,
    TaskRunner,
};

use super::Executor;

/// Executes each task on a dedicated OS thread.
///
/// This executor does not manage lifecycle or maintain a queue. Each accepted
/// task receives a [`TaskHandle`] that can be used to wait for the result.
///
/// # Semantics
///
/// * **One task, one thread** — each [`Executor::call`] or [`Executor::execute`]
///   spawns a new [`std::thread::spawn`] worker. There is no pool and no
///   submission queue.
/// * **Blocking or async wait** — [`TaskHandle::get`] blocks the calling thread,
///   while awaiting the handle uses a waker and does not block the polling
///   thread.
/// * **Completion probe** — [`TaskHandle::is_done`] reads an atomic flag set
///   after the worker publishes the result; it does not retrieve the value
///   (you still need [`TaskHandle::get`] for that).
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
/// let executor = ThreadPerTaskExecutor;
/// let handle = executor.call(|| Ok::<i32, io::Error>(40 + 2));
///
/// // Blocks the current thread until the spawned thread completes.
/// let value = handle.get().expect("task should succeed");
/// assert_eq!(value, 42);
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct ThreadPerTaskExecutor;

impl Executor for ThreadPerTaskExecutor {
    type Execution<R, E>
        = TaskHandle<R, E>
    where
        R: Send + 'static,
        E: std::fmt::Display + Send + 'static;

    /// Spawns one OS thread for the callable and returns a handle to its result.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to run on a dedicated OS thread.
    ///
    /// # Returns
    ///
    /// A [`TaskHandle`] that can block or await the spawned task's final
    /// result.
    fn call<C, R, E>(&self, task: C) -> Self::Execution<R, E>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        let (handle, completion) = TaskCompletionPair::new().into_parts();
        thread::spawn(move || {
            TaskRunner::new(task).run(completion);
        });
        handle
    }
}
