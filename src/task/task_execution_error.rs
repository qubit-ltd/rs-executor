// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::error::Error;
use std::fmt;

/// Result type used by managed task handles.
pub type TaskResult<R, E> = Result<R, TaskExecutionError<E>>;

/// Error observed when retrieving the result of an accepted task.
///
/// This error is distinct from [`SubmissionError`](crate::SubmissionError).
/// Rejection happens before a service accepts a task; `TaskExecutionError`
/// describes what happened after the task was accepted.
///
/// # Type Parameters
///
/// * `E` - The error type returned by the task itself.
#[derive(Debug)]
pub enum TaskExecutionError<E> {
    /// The task ran and returned `Err(E)`.
    Failed(E),

    /// The task panicked while running.
    Panicked,

    /// The task was explicitly cancelled before producing a result.
    ///
    /// This includes caller-side cancellation through tracked handles and
    /// executor/service-side cancellation of queued, scheduled, or otherwise
    /// unstarted work.
    Cancelled,

    /// The accepted runner-side completion endpoint was abandoned without
    /// publishing an explicit terminal result.
    ///
    /// This represents runner loss or misuse. Services that intentionally stop
    /// unstarted accepted work should publish [`Self::Cancelled`] instead.
    Dropped,
}

impl<E> TaskExecutionError<E> {
    /// Returns true when this error wraps the task's own error value.
    ///
    /// # Returns
    ///
    /// `true` if the task returned `Err(E)`.
    #[inline]
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// Returns true when the task panicked.
    ///
    /// # Returns
    ///
    /// `true` if the task panicked while running.
    #[inline]
    pub const fn is_panicked(&self) -> bool {
        matches!(self, Self::Panicked)
    }

    /// Returns true when the task was explicitly cancelled.
    ///
    /// # Returns
    ///
    /// `true` if the task was cancelled before producing a result.
    #[inline]
    pub const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    /// Returns true when the task result was abandoned by the completion
    /// endpoint.
    ///
    /// # Returns
    ///
    /// `true` if the accepted runner-side completion endpoint disappeared
    /// without publishing an explicit terminal result.
    #[inline]
    pub const fn is_dropped(&self) -> bool {
        matches!(self, Self::Dropped)
    }
}

impl<E> fmt::Display for TaskExecutionError<E>
where
    E: fmt::Display,
{
    /// Formats this task execution error for users.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(err) => write!(f, "task failed: {err}"),
            Self::Panicked => f.write_str("task panicked"),
            Self::Cancelled => f.write_str("task was cancelled"),
            Self::Dropped => f.write_str("task result was dropped"),
        }
    }
}

impl<E> Error for TaskExecutionError<E> where E: Error + 'static {}
