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
    future::Future,
    pin::Pin,
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicU8, Ordering},
    },
    thread,
};

use qubit_atomic::AtomicCount;
use qubit_function::{Callable, Runnable};

use crate::{TaskCompletionPair, TaskHandle, TaskRunner, TrackedTask};

use super::{ExecutorService, ExecutorServiceLifecycle, RejectedExecution, StopReport};

/// Shared state for [`ThreadPerTaskExecutorService`].
#[derive(Default)]
struct ThreadPerTaskExecutorServiceState {
    /// Current lifecycle state encoded as an [`ExecutorServiceLifecycle`] discriminant.
    lifecycle: AtomicU8,
    /// Number of accepted OS-thread tasks that have not completed.
    active_tasks: AtomicCount,
    /// Serializes task submission and shutdown transitions.
    submission_lock: Mutex<()>,
    /// Mutex paired with the termination condition variable.
    termination_lock: Mutex<()>,
    /// Condition variable used to wait for service termination.
    termination: Condvar,
}

impl ThreadPerTaskExecutorServiceState {
    /// Acquires the submission lock while tolerating poisoned locks.
    ///
    /// # Returns
    ///
    /// A guard for the submission lock.
    #[inline]
    fn lock_submission(&self) -> MutexGuard<'_, ()> {
        self.submission_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Acquires the termination lock while tolerating poisoned locks.
    ///
    /// # Returns
    ///
    /// A guard for the mutex paired with the termination condition variable.
    #[inline]
    fn lock_termination(&self) -> MutexGuard<'_, ()> {
        self.termination_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Returns the currently stored lifecycle state.
    ///
    /// # Returns
    ///
    /// The lifecycle represented by the internal atomic discriminant.
    #[inline]
    fn lifecycle(&self) -> ExecutorServiceLifecycle {
        ExecutorServiceLifecycle::from_u8(self.lifecycle.load(Ordering::Acquire))
    }

    /// Stores a new lifecycle state.
    ///
    /// # Parameters
    ///
    /// * `lifecycle` - New lifecycle state to publish.
    #[inline]
    fn set_lifecycle(&self, lifecycle: ExecutorServiceLifecycle) {
        self.lifecycle.store(lifecycle as u8, Ordering::Release);
    }

    /// Wakes termination waiters when shutdown and task completion allow it.
    #[inline]
    fn notify_if_terminated(&self) {
        if self.lifecycle() != ExecutorServiceLifecycle::Running && self.active_tasks.is_zero() {
            self.set_lifecycle(ExecutorServiceLifecycle::Terminated);
            self.termination.notify_all();
        }
    }

    /// Blocks the current thread until the service is terminated.
    fn wait_for_termination(&self) {
        let mut guard = self.lock_termination();
        while self.lifecycle() != ExecutorServiceLifecycle::Terminated {
            guard = self
                .termination
                .wait(guard)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
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

    type Termination<'a>
        = Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    where
        Self: 'a;

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
    /// Returns [`RejectedExecution::Shutdown`] if shutdown has already been
    /// requested before the task is accepted.
    fn submit<T, E>(&self, task: T) -> Result<(), RejectedExecution>
    where
        T: Runnable<E> + Send + 'static,
        E: Send + 'static,
    {
        let submission_guard = self.state.lock_submission();
        if self.state.lifecycle() != ExecutorServiceLifecycle::Running {
            return Err(RejectedExecution::Shutdown);
        }
        self.state.active_tasks.inc();
        drop(submission_guard);

        let state = Arc::clone(&self.state);
        thread::spawn(move || {
            let mut task = task;
            let _ignored = TaskRunner::new(move || task.run()).call::<(), E>();
            if state.active_tasks.dec() == 0 {
                state.notify_if_terminated();
            }
        });
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
    /// Returns [`RejectedExecution::Shutdown`] if shutdown has already been
    /// requested before the task is accepted.
    fn submit_callable<C, R, E>(
        &self,
        task: C,
    ) -> Result<Self::ResultHandle<R, E>, RejectedExecution>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let submission_guard = self.state.lock_submission();
        if self.state.lifecycle() != ExecutorServiceLifecycle::Running {
            return Err(RejectedExecution::Shutdown);
        }
        self.state.active_tasks.inc();
        drop(submission_guard);

        let (handle, completion) = TaskCompletionPair::new().into_parts();
        let state = Arc::clone(&self.state);
        thread::spawn(move || {
            TaskRunner::new(task).run(completion);
            if state.active_tasks.dec() == 0 {
                state.notify_if_terminated();
            }
        });
        Ok(handle)
    }

    /// Accepts a callable and starts it with a tracked handle.
    fn submit_tracked_callable<C, R, E>(
        &self,
        task: C,
    ) -> Result<Self::TrackedHandle<R, E>, RejectedExecution>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let submission_guard = self.state.lock_submission();
        if self.state.lifecycle() != ExecutorServiceLifecycle::Running {
            return Err(RejectedExecution::Shutdown);
        }
        self.state.active_tasks.inc();
        drop(submission_guard);

        let (handle, completion) = TaskCompletionPair::new().into_tracked_parts();
        let state = Arc::clone(&self.state);
        thread::spawn(move || {
            TaskRunner::new(task).run(completion);
            if state.active_tasks.dec() == 0 {
                state.notify_if_terminated();
            }
        });
        Ok(handle)
    }

    /// Stops accepting new tasks.
    ///
    /// Already accepted threads are allowed to finish.
    fn shutdown(&self) {
        let _guard = self.state.lock_submission();
        if self.state.lifecycle() == ExecutorServiceLifecycle::Running {
            self.state
                .set_lifecycle(ExecutorServiceLifecycle::ShuttingDown);
        }
        self.state.notify_if_terminated();
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
        let _guard = self.state.lock_submission();
        if self.state.lifecycle() != ExecutorServiceLifecycle::Terminated {
            self.state.set_lifecycle(ExecutorServiceLifecycle::Stopping);
        }
        let running = self.state.active_tasks.get();
        self.state.notify_if_terminated();
        StopReport::new(0, running, 0)
    }

    /// Returns the current lifecycle state.
    #[inline]
    fn lifecycle(&self) -> ExecutorServiceLifecycle {
        self.state.lifecycle()
    }

    /// Waits for all accepted tasks to complete after shutdown.
    ///
    /// This future blocks the polling thread while waiting on a condition
    /// variable.
    ///
    /// # Returns
    ///
    /// A future that resolves after shutdown has been requested and all
    /// accepted OS-thread tasks have completed.
    #[inline]
    fn await_termination(&self) -> Self::Termination<'_> {
        Box::pin(async move {
            self.state.wait_for_termination();
        })
    }
}
