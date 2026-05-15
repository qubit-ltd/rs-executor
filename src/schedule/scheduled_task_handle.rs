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
    future::IntoFuture,
    sync::Arc,
};

use qubit_atomic::Atomic;

use crate::{
    CancelResult,
    TaskResult,
    TaskStatus,
    TrackedTask,
    TryGet,
    hook::TaskId,
    task::{
        TaskHandleFuture,
        spi::{
            TaskResultHandle,
            TrackedTaskHandle,
        },
    },
};

/// Tracked handle for a task accepted by a scheduled executor service.
///
/// The handle delegates result and status observation to the standard
/// [`TrackedTask`] while also waking the scheduler when pre-start cancellation
/// wins. This prevents a cancelled timer entry from keeping the single scheduler
/// thread asleep until the original deadline.
pub struct ScheduledTaskHandle<R, E> {
    /// Standard tracked task handle.
    inner: TrackedTask<R, E>,
    /// Shared marker observed by the scheduler heap.
    cancellation_marker: Arc<Atomic<bool>>,
    /// Callback invoked after this handle cancels the pending task.
    on_cancelled: Arc<dyn Fn() + Send + Sync + 'static>,
}

impl<R, E> ScheduledTaskHandle<R, E> {
    /// Creates a scheduled task handle.
    ///
    /// # Parameters
    ///
    /// * `inner` - Standard tracked task handle.
    /// * `cancellation_marker` - Shared marker observed by the scheduler heap.
    /// * `on_cancelled` - Callback invoked when this handle cancels the task.
    ///
    /// # Returns
    ///
    /// A scheduled task handle.
    pub(crate) const fn new(
        inner: TrackedTask<R, E>,
        cancellation_marker: Arc<Atomic<bool>>,
        on_cancelled: Arc<dyn Fn() + Send + Sync + 'static>,
    ) -> Self {
        Self {
            inner,
            cancellation_marker,
            on_cancelled,
        }
    }

    /// Waits for the task to finish and returns its final result.
    ///
    /// # Returns
    ///
    /// The final task result.
    #[inline]
    pub fn get(self) -> TaskResult<R, E>
    where
        R: Send,
        E: Send,
    {
        <Self as TaskResultHandle<R, E>>::get(self)
    }

    /// Attempts to retrieve the final result without blocking.
    ///
    /// # Returns
    ///
    /// A ready result or the pending scheduled handle.
    #[inline]
    pub fn try_get(self) -> TryGet<Self, R, E>
    where
        R: Send,
        E: Send,
    {
        <Self as TaskResultHandle<R, E>>::try_get(self)
    }

    /// Returns whether the tracked task has installed a terminal state.
    ///
    /// # Returns
    ///
    /// `true` after the task succeeds, fails, panics, is cancelled, or loses its
    /// completion endpoint.
    #[inline]
    pub fn is_done(&self) -> bool
    where
        R: Send,
        E: Send,
    {
        <Self as TaskResultHandle<R, E>>::is_done(self)
    }

    /// Returns the currently observed task status.
    ///
    /// # Returns
    ///
    /// The current task status.
    #[inline]
    pub fn status(&self) -> TaskStatus {
        self.inner.status()
    }

    /// Returns the identifier assigned to this task.
    ///
    /// # Returns
    ///
    /// The task id stored in the shared task state.
    #[inline]
    pub fn task_id(&self) -> TaskId {
        self.inner.task_id()
    }

    /// Attempts to cancel this task before it starts.
    ///
    /// # Returns
    ///
    /// The observed cancellation outcome.
    #[inline]
    pub fn cancel(&self) -> CancelResult {
        self.cancel_inner()
    }

    /// Performs cancellation and wakes the scheduler if this handle won.
    ///
    /// # Returns
    ///
    /// The observed cancellation outcome.
    fn cancel_inner(&self) -> CancelResult {
        let result = self.inner.cancel();
        if result == CancelResult::Cancelled {
            self.cancellation_marker.store(true);
            (self.on_cancelled)();
        }
        result
    }
}

impl<R, E> TaskResultHandle<R, E> for ScheduledTaskHandle<R, E>
where
    R: Send,
    E: Send,
{
    /// Returns whether the tracked state is terminal.
    #[inline]
    fn is_done(&self) -> bool {
        self.inner.is_done()
    }

    /// Blocks until the underlying result handle yields a result.
    #[inline]
    fn get(self) -> TaskResult<R, E> {
        self.inner.get()
    }

    /// Attempts to retrieve the underlying result without blocking.
    #[inline]
    fn try_get(self) -> TryGet<Self, R, E> {
        let Self {
            inner,
            cancellation_marker,
            on_cancelled,
        } = self;
        match inner.try_get() {
            TryGet::Ready(result) => TryGet::Ready(result),
            TryGet::Pending(inner) => TryGet::Pending(Self {
                inner,
                cancellation_marker,
                on_cancelled,
            }),
        }
    }
}

impl<R, E> TrackedTaskHandle<R, E> for ScheduledTaskHandle<R, E>
where
    R: Send,
    E: Send,
{
    /// Returns the currently observed task status.
    #[inline]
    fn status(&self) -> TaskStatus {
        self.inner.status()
    }

    /// Attempts to publish a cancellation result while the task is pending.
    #[inline]
    fn cancel(&self) -> CancelResult {
        self.cancel_inner()
    }
}

impl<R, E> IntoFuture for ScheduledTaskHandle<R, E> {
    type Output = TaskResult<R, E>;
    type IntoFuture = TaskHandleFuture<R, E>;

    /// Converts this scheduled handle into a future resolving to the task result.
    #[inline]
    fn into_future(self) -> Self::IntoFuture {
        self.inner.into_future()
    }
}
