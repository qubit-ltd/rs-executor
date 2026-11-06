// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::single_thread_scheduled_executor_service::SingleThreadScheduledExecutorService;
use crate::service::ExecutorServiceBuilderError;

/// Builder for a single-thread scheduled executor service.
///
/// The queue is unbounded unless [`queue_capacity`](Self::queue_capacity) is
/// configured.
///
/// # Examples
///
/// ```rust
/// use qubit_executor::{ExecutorService, SingleThreadScheduledExecutorService};
///
/// let service = SingleThreadScheduledExecutorService::builder("bounded-scheduler")
///     .queue_capacity(128)
///     .build()
///     .expect("scheduler should start");
/// service.stop();
/// service.wait_termination();
/// ```
pub struct SingleThreadScheduledExecutorServiceBuilder {
    thread_name: String,
    stack_size: Option<usize>,
    queue_capacity: Option<usize>,
}

impl SingleThreadScheduledExecutorServiceBuilder {
    /// Creates a builder for a scheduler thread with the given name.
    pub(super) fn new(thread_name: &str) -> Self {
        Self {
            thread_name: thread_name.to_owned(),
            stack_size: None,
            queue_capacity: None,
        }
    }

    /// Sets the scheduler thread stack size.
    ///
    /// # Parameters
    ///
    /// * `stack_size` - Requested stack size in bytes; must be greater than zero.
    ///
    /// # Returns
    ///
    /// The builder with the requested scheduler thread stack size.
    #[must_use]
    pub fn stack_size(mut self, stack_size: usize) -> Self {
        self.stack_size = Some(stack_size);
        self
    }

    /// Limits the number of tasks waiting in the scheduler queue.
    ///
    /// The running task does not count toward this limit. If the queue is full,
    /// submissions return [`crate::SubmissionError::Saturated`]. Calling this
    /// method again replaces the previous capacity.
    ///
    /// # Parameters
    ///
    /// * `queue_capacity` - Maximum queued task count; must be greater than zero.
    ///
    /// # Returns
    ///
    /// The builder with the requested maximum number of queued tasks.
    #[must_use]
    pub fn queue_capacity(mut self, queue_capacity: usize) -> Self {
        self.queue_capacity = Some(queue_capacity);
        self
    }

    /// Starts the configured scheduled service.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutorServiceBuilderError::ZeroStackSize`] or
    /// [`ExecutorServiceBuilderError::ZeroQueueCapacity`] for an explicit zero
    /// setting, or [`ExecutorServiceBuilderError::SpawnWorker`] if its worker
    /// cannot be created.
    ///
    /// # Panics
    ///
    /// Panics if the thread name contains a NUL byte.
    pub fn build(
        self,
    ) -> Result<SingleThreadScheduledExecutorService, ExecutorServiceBuilderError> {
        if self.stack_size == Some(0) {
            return Err(ExecutorServiceBuilderError::ZeroStackSize);
        }
        if self.queue_capacity == Some(0) {
            return Err(ExecutorServiceBuilderError::ZeroQueueCapacity);
        }
        SingleThreadScheduledExecutorService::with_config(
            &self.thread_name,
            self.stack_size,
            self.queue_capacity,
        )
    }
}
