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
use std::time::Instant;

use qubit_function::Callable;
use qubit_function::Runnable;

use super::completable_scheduled_task::CompletableScheduledTask;
use super::scheduled_executor_service::ScheduledExecutorService;
use super::scheduled_task_entry::ScheduledTaskEntry;
use super::scheduled_task_handle::ScheduledTaskHandle;
use super::scheduled_worker::ScheduledWorker;
use super::scheduler_core::SchedulerCore;
use super::single_thread_scheduled_executor_service_builder::SingleThreadScheduledExecutorServiceBuilder;
use crate::TaskHandle;
use crate::service::ExecutorService;
use crate::service::ExecutorServiceBuilderError;
use crate::service::ExecutorServiceLifecycle;
use crate::service::StopReport;
use crate::service::SubmissionError;
use crate::task::spi::TaskEndpointPair;

/// Single-threaded scheduled executor service.
///
/// The service owns one scheduler OS thread. It accepts tasks into a deadline
/// heap, waits until the earliest task is due, and then runs that task directly
/// on the scheduler thread. Scheduled tasks should therefore stay short; submit
/// heavier work to another executor service from the scheduled task body.
/// If the background timer wait fails, the service stops automatically, cancels
/// tasks that have not started, and rejects later submissions with
/// [`SubmissionError::Shutdown`]. This recovers ordinary unwinding only; it
/// cannot recover from aborting panics or permanently blocking user cleanup.
///
/// `stop` cancels entries still queued when they are drained. A task already
/// claimed by the worker may still start after `stop` returns. The queue and
/// active counts describe scheduler entries and worker activity, so they need
/// not match each handle's `TaskStatus` at every instant. Termination waiting
/// observes the scheduler service's termination condition; it does not join
/// every OS thread or wait for unrelated concurrent cancellation cleanup.
///
/// # Examples
///
/// ```rust
/// use std::time::Duration;
///
/// use qubit_executor::{ExecutorService, ScheduledExecutorService, SingleThreadScheduledExecutorService};
///
/// let service = SingleThreadScheduledExecutorService::new("example-scheduler")
///     .expect("scheduler thread should start");
/// let task = service
///     .schedule_callable(Duration::from_millis(1), || Ok::<_, ()>(42))
///     .expect("scheduled task should be accepted");
/// assert_eq!(task.get().expect("task should succeed"), 42);
/// service.shutdown();
/// service.wait_termination();
/// ```
pub struct SingleThreadScheduledExecutorService {
    /// Shared scheduler state.
    inner: Arc<SchedulerCore>,
}

impl SingleThreadScheduledExecutorService {
    /// Starts a new single-thread scheduled executor service.
    ///
    /// # Parameters
    ///
    /// * `thread_name` - Name for the scheduler thread; must not contain a NUL
    ///   byte.
    ///
    /// # Returns
    ///
    /// A started scheduled executor service.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutorServiceBuilderError::SpawnWorker`] if the scheduler
    /// thread cannot be created.
    ///
    /// # Panics
    ///
    /// Panics if `thread_name` contains a NUL byte.
    #[inline]
    pub fn new(thread_name: &str) -> Result<Self, ExecutorServiceBuilderError> {
        Self::with_stack_size(thread_name, None)
    }

    /// Creates a builder for a scheduled service.
    ///
    /// The builder defaults to an unbounded queue. Configure
    /// [`SingleThreadScheduledExecutorServiceBuilder::queue_capacity`] to
    /// reject submissions with [`SubmissionError::Saturated`] when the
    /// configured number of queued entries has been reached.
    ///
    /// # Parameters
    ///
    /// * `thread_name` - Name for the scheduler thread; must not contain a NUL
    ///   byte.
    ///
    /// # Returns
    ///
    /// A builder with no stack-size or queue-capacity limit configured.
    #[must_use]
    pub fn builder(thread_name: &str) -> SingleThreadScheduledExecutorServiceBuilder {
        SingleThreadScheduledExecutorServiceBuilder::new(thread_name)
    }

    /// Starts a new scheduled service with an optional scheduler thread stack
    /// size.
    ///
    /// # Parameters
    ///
    /// * `thread_name` - Name for the scheduler thread; must not contain a NUL
    ///   byte.
    /// * `stack_size` - Optional stack size for the scheduler thread.
    ///
    /// # Returns
    ///
    /// A started scheduled executor service.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutorServiceBuilderError::SpawnWorker`] if the scheduler
    /// thread cannot be created.
    ///
    /// # Panics
    ///
    /// Panics if `thread_name` contains a NUL byte.
    pub fn with_stack_size(thread_name: &str, stack_size: Option<usize>) -> Result<Self, ExecutorServiceBuilderError> {
        Self::with_config(thread_name, stack_size, None)
    }

    /// Starts a scheduler using explicit thread and queue configuration.
    pub(super) fn with_config(
        thread_name: &str,
        stack_size: Option<usize>,
        queue_capacity: Option<usize>,
    ) -> Result<Self, ExecutorServiceBuilderError> {
        let inner = Arc::new(SchedulerCore::with_queue_capacity(queue_capacity));
        let worker_inner = Arc::clone(&inner);
        let mut builder = thread::Builder::new().name(thread_name.to_string());
        if let Some(stack_size) = stack_size {
            builder = builder.stack_size(stack_size);
        }
        if let Err(source) = builder.spawn(move || ScheduledWorker::run(worker_inner)) {
            return Err(ExecutorServiceBuilderError::SpawnWorker { index: Some(0), source });
        }
        Ok(Self { inner })
    }

    /// Returns the number of queued scheduled tasks.
    ///
    /// # Returns
    ///
    /// Number of accepted scheduled entries that remain in the deadline queue.
    /// A worker-claimed entry is no longer queued, even if its handle still
    /// reports `Pending` while the worker is about to start it.
    #[must_use]
    #[inline]
    pub fn queued_count(&self) -> usize {
        self.inner.queued_count()
    }

    /// Returns the number of currently running tasks.
    ///
    /// # Returns
    ///
    /// `1` while the scheduler worker owns a claimed entry, otherwise `0`.
    /// This may include the interval before the handle changes from `Pending`
    /// to `Running`, and does not count concurrent cancellation cleanup on
    /// another thread.
    #[must_use]
    #[inline]
    pub fn running_count(&self) -> usize {
        self.inner.running_count()
    }

    /// Creates a cancellation callback for a scheduled task identifier.
    ///
    /// # Returns
    ///
    /// Callback that removes a queued task and wakes the scheduler when its
    /// handle cancels before start.
    ///
    /// # Parameters
    ///
    /// * `task_id` - Identifier of the scheduled task to cancel.
    fn cancellation_callback(&self, task_id: crate::hook::TaskId) -> Arc<dyn Fn() + Send + Sync + 'static> {
        let inner = Arc::downgrade(&self.inner);
        Arc::new(move || {
            if let Some(inner) = inner.upgrade() {
                inner.cancel_queued_task(task_id);
            }
        })
    }

    /// Accepts a type-erased task into the deadline-ordered scheduler map.
    ///
    /// # Parameters
    ///
    /// * `deadline` - Monotonic instant when the task becomes runnable.
    /// * `entry` - Type-erased scheduled task entry.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::Shutdown`] after shutdown or stop starts.
    /// Returns [`SubmissionError::Saturated`] when a configured queue is full.
    fn schedule_entry(
        &self,
        task_id: crate::hook::TaskId,
        deadline: Instant,
        entry: Box<dyn ScheduledTaskEntry>,
    ) -> Result<(), SubmissionError> {
        self.inner.schedule(task_id, deadline, entry)
    }

    /// Schedules a callable and returns a standard result handle.
    ///
    /// # Parameters
    ///
    /// * `deadline` - Monotonic instant when the task becomes runnable.
    /// * `task` - Callable task to execute.
    ///
    /// # Returns
    ///
    /// A task result handle for the accepted task.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::Shutdown`] after shutdown or stop starts.
    /// Returns [`SubmissionError::Saturated`] when a configured queue is full.
    ///
    /// # Type Parameters
    ///
    /// * `C` - Callable that produces the task result.
    /// * `R` - Successful task result type.
    /// * `E` - Task error type.
    fn schedule_result_handle<C, R, E>(&self, deadline: Instant, task: C) -> Result<TaskHandle<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let (handle, slot) = TaskEndpointPair::new().into_parts();
        let task_id = handle.task_id();
        let entry = CompletableScheduledTask::new(task, slot);
        self.schedule_entry(task_id, deadline, Box::new(entry))?;
        Ok(handle)
    }
}

impl Drop for SingleThreadScheduledExecutorService {
    /// Requests graceful shutdown when the service handle is dropped.
    fn drop(&mut self) {
        self.inner.shutdown();
    }
}

impl ExecutorService for SingleThreadScheduledExecutorService {
    /// Standard result handle returned by immediate task submission.
    type ResultHandle<R, E>
        = TaskHandle<R, E>
    where
        R: Send + 'static,
        E: Send + 'static;

    /// Scheduled handle that can cancel an accepted task before it starts.
    type TrackedHandle<R, E>
        = ScheduledTaskHandle<R, E>
    where
        R: Send + 'static,
        E: Send + 'static;

    /// Accepts a runnable for immediate execution on the scheduler thread.
    ///
    /// # Parameters
    ///
    /// * `task` - Runnable task to execute.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Runnable task type.
    /// * `E` - Error type returned by the runnable.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the task was accepted.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::Shutdown`] if the service has stopped
    /// accepting tasks, or [`SubmissionError::Saturated`] when a configured
    /// queue is full.
    fn submit<T, E>(&self, task: T) -> Result<(), SubmissionError>
    where
        T: Runnable<E> + Send + 'static,
        E: Send + 'static,
    {
        let mut task = task;
        let handle = self.submit_callable(move || task.run())?;
        drop(handle);
        Ok(())
    }

    /// Accepts a callable for immediate execution on the scheduler thread.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable task to execute.
    ///
    /// # Type Parameters
    ///
    /// * `C` - Callable that produces the task result.
    /// * `R` - Successful task result type.
    /// * `E` - Task error type.
    ///
    /// # Returns
    ///
    /// A standard result handle for the accepted task.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::Shutdown`] if the service has stopped
    /// accepting tasks, or [`SubmissionError::Saturated`] when a configured
    /// queue is full.
    fn submit_callable<C, R, E>(&self, task: C) -> Result<Self::ResultHandle<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        self.schedule_result_handle(Instant::now(), task)
    }

    /// Accepts a callable for immediate execution with a scheduled task handle.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable task to execute.
    ///
    /// # Type Parameters
    ///
    /// * `C` - Callable that produces the task result.
    /// * `R` - Successful task result type.
    /// * `E` - Task error type.
    ///
    /// # Returns
    ///
    /// A tracked handle that can cancel the task before it starts.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::Shutdown`] if the service has stopped
    /// accepting tasks, or [`SubmissionError::Saturated`] when a configured
    /// queue is full.
    fn submit_tracked_callable<C, R, E>(&self, task: C) -> Result<Self::TrackedHandle<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        self.schedule_callable_at(Instant::now(), task)
    }

    /// Requests graceful shutdown and drains accepted scheduled work.
    ///
    /// Queued tasks retain their original deadlines after shutdown. Termination
    /// may therefore remain pending even when no task is currently running.
    /// Use [`ExecutorService::stop`] to cancel entries still queued, or cancel
    /// individual tracked handles before they start. A termination-wait timeout
    /// does not itself stop the service.
    #[inline]
    fn shutdown(&self) {
        self.inner.shutdown();
    }

    /// Requests immediate shutdown and cancels entries still queued.
    ///
    /// This implementation cancels drained queued tasks synchronously on the
    /// calling thread. Destruction of captured values can block. A task already
    /// claimed by the worker is outside this queue drain and may still start;
    /// running tasks are not forcibly interrupted.
    ///
    /// # Panics
    ///
    /// If cancellation or destruction of a drained task unwinds, this
    /// implementation continues processing the remaining drained entries,
    /// completes its drain bookkeeping, and then resumes the first captured
    /// panic. This does not recover from aborting panics or process
    /// termination.
    #[inline]
    fn stop(&self) -> StopReport {
        self.inner.stop()
    }

    /// Returns the current lifecycle state.
    #[inline]
    fn lifecycle(&self) -> ExecutorServiceLifecycle {
        self.inner.lifecycle()
    }

    /// Returns whether shutdown has started.
    #[inline]
    fn is_not_running(&self) -> bool {
        self.inner.is_not_running()
    }

    /// Returns whether the scheduler worker has published the service's
    /// termination condition.
    ///
    /// This does not join the OS thread or wait for a concurrent cancellation
    /// call to finish destroying captured values.
    #[inline]
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }

    /// Blocks until this scheduled service reports termination.
    ///
    /// This waits for the worker's service condition, not for an OS-thread
    /// join, thread-local destructors, or concurrent cancellation cleanup.
    #[inline]
    fn wait_termination(&self) {
        self.inner.wait_for_termination();
    }

    /// Waits at most `timeout` for the scheduler service to report termination.
    ///
    /// Returns `true` when the worker publishes the service's termination
    /// condition. This does not join the OS thread or wait for cleanup running
    /// in a concurrent cancellation call.
    ///
    /// # Parameters
    ///
    /// * `timeout` - Maximum time to wait for the service termination
    ///   condition.
    #[inline]
    fn wait_termination_timeout(&self, timeout: Duration) -> bool {
        self.inner.wait_for_termination_timeout(timeout)
    }
}

impl ScheduledExecutorService for SingleThreadScheduledExecutorService {
    /// Schedules a callable task to start at a monotonic instant.
    ///
    /// # Parameters
    ///
    /// * `instant` - Monotonic instant when the task may start.
    /// * `task` - Callable task to execute.
    ///
    /// # Type Parameters
    ///
    /// * `C` - Callable that produces the task result.
    /// * `R` - Successful task result type.
    /// * `E` - Task error type.
    ///
    /// # Returns
    ///
    /// A tracked handle for observing or cancelling the task.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::Shutdown`] if the service has stopped
    /// accepting tasks, or [`SubmissionError::Saturated`] when a configured
    /// queue is full.
    fn schedule_callable_at<C, R, E>(
        &self,
        instant: Instant,
        task: C,
    ) -> Result<Self::TrackedHandle<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let (tracked, slot) = TaskEndpointPair::new().into_tracked_parts();
        let task_id = tracked.task_id();
        let cancellation_callback = self.cancellation_callback(task_id);
        let entry = CompletableScheduledTask::new(task, slot);
        self.schedule_entry(task_id, instant, Box::new(entry))?;
        Ok(ScheduledTaskHandle::new(tracked, cancellation_callback))
    }
}
