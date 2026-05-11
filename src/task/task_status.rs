/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use super::{
    TaskExecutionError,
    TaskResult,
};

/// Observable lifecycle status for a submitted task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// The task has been accepted but has not started running.
    Pending,
    /// The task has started running.
    Running,
    /// The task completed successfully.
    Succeeded,
    /// The task returned its own error value.
    Failed,
    /// The task panicked while running.
    Panicked,
    /// The task was cancelled before producing a value.
    Cancelled,
    /// The accepted runner-side completion endpoint was dropped before
    /// producing a value.
    Dropped,
}

impl TaskStatus {
    /// Returns whether this status is terminal.
    ///
    /// # Returns
    ///
    /// `true` after success, failure, panic, cancellation, or dropped
    /// completion.
    #[inline]
    pub const fn is_done(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Panicked | Self::Cancelled | Self::Dropped
        )
    }

    /// Converts this status to its compact atomic representation.
    ///
    /// # Returns
    ///
    /// A stable byte code used by task completion state.
    #[inline]
    pub(crate) const fn as_u8(self) -> u8 {
        match self {
            Self::Pending => 0,
            Self::Running => 1,
            Self::Succeeded => 2,
            Self::Failed => 3,
            Self::Panicked => 4,
            Self::Cancelled => 5,
            Self::Dropped => 6,
        }
    }

    /// Converts a compact atomic representation into a task status.
    ///
    /// # Parameters
    ///
    /// * `value` - Byte value previously produced by [`Self::as_u8`].
    ///
    /// # Returns
    ///
    /// The represented task status.
    ///
    /// # Panics
    ///
    /// Panics if `value` is not a valid task status code.
    #[inline]
    pub(crate) const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Pending,
            1 => Self::Running,
            2 => Self::Succeeded,
            3 => Self::Failed,
            4 => Self::Panicked,
            5 => Self::Cancelled,
            6 => Self::Dropped,
            _ => panic!("invalid task status code"),
        }
    }

    /// Returns the terminal status represented by a task result.
    ///
    /// # Parameters
    ///
    /// * `result` - Final task result being published.
    ///
    /// # Returns
    ///
    /// The terminal status matching `result`.
    #[inline]
    pub(crate) const fn from_result<R, E>(result: &TaskResult<R, E>) -> Self {
        match result {
            Ok(_) => Self::Succeeded,
            Err(TaskExecutionError::Failed(_)) => Self::Failed,
            Err(TaskExecutionError::Panicked) => Self::Panicked,
            Err(TaskExecutionError::Cancelled) => Self::Cancelled,
            Err(TaskExecutionError::Dropped) => Self::Dropped,
        }
    }
}
