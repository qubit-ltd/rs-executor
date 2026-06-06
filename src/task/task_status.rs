// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow inline-tests
use core::mem::transmute;

use super::{
    TaskExecutionError,
    TaskResult,
};

/// Number of [`TaskStatus`] variants; compact codes are `0..TASK_STATUS_COUNT`.
pub(crate) const TASK_STATUS_COUNT: usize = 7;

/// Observable lifecycle status for a submitted task.
///
/// `#[repr(usize)]` assigns stable discriminants `0..TASK_STATUS_COUNT` for
/// internal compact state-machine encoding.
#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// The task has been accepted but has not started running.
    Pending = 0,
    /// The task has started running.
    Running = 1,
    /// The task completed successfully.
    Succeeded = 2,
    /// The task returned its own error value.
    Failed = 3,
    /// The task panicked while running.
    Panicked = 4,
    /// The task was explicitly cancelled before producing a value.
    Cancelled = 5,
    /// The accepted runner-side completion endpoint was abandoned before
    /// producing an explicit terminal value.
    Dropped = 6,
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
            Self::Succeeded
                | Self::Failed
                | Self::Panicked
                | Self::Cancelled
                | Self::Dropped
        )
    }

    /// Converts this status to its compact state-machine representation.
    ///
    /// # Returns
    ///
    /// A stable integer code used by task completion state.
    #[inline]
    pub(crate) const fn as_usize(self) -> usize {
        self as usize
    }

    /// Converts a compact state-machine representation into a task status.
    ///
    /// # Parameters
    ///
    /// * `value` - Integer value previously produced by [`Self::as_usize`].
    ///
    /// # Returns
    ///
    /// The represented task status.
    ///
    /// # Panics
    ///
    /// Panics if `value` is not a valid task status code.
    #[inline]
    pub(crate) const fn from_usize(value: usize) -> Self {
        if value >= TASK_STATUS_COUNT {
            panic!("invalid task status code");
        }
        unsafe { transmute::<usize, Self>(value) }
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

#[cfg(test)]
mod compact_encoding_tests {
    use super::{
        TASK_STATUS_COUNT,
        TaskStatus,
    };

    #[test]
    fn task_status_as_usize_matches_stable_discriminants() {
        assert_eq!(TaskStatus::Pending.as_usize(), 0);
        assert_eq!(TaskStatus::Running.as_usize(), 1);
        assert_eq!(TaskStatus::Succeeded.as_usize(), 2);
        assert_eq!(TaskStatus::Failed.as_usize(), 3);
        assert_eq!(TaskStatus::Panicked.as_usize(), 4);
        assert_eq!(TaskStatus::Cancelled.as_usize(), 5);
        assert_eq!(TaskStatus::Dropped.as_usize(), 6);
    }

    #[test]
    fn task_status_from_usize_restores_each_variant() {
        let variants = [
            TaskStatus::Pending,
            TaskStatus::Running,
            TaskStatus::Succeeded,
            TaskStatus::Failed,
            TaskStatus::Panicked,
            TaskStatus::Cancelled,
            TaskStatus::Dropped,
        ];
        for status in variants {
            assert_eq!(TaskStatus::from_usize(status.as_usize()), status);
        }
    }

    #[test]
    fn task_status_round_trip_all_codes_in_range() {
        for code in 0..TASK_STATUS_COUNT {
            let status = TaskStatus::from_usize(code);
            assert_eq!(status.as_usize(), code);
            assert_eq!(TaskStatus::from_usize(code), status);
        }
    }

    #[test]
    #[should_panic(expected = "invalid task status code")]
    fn task_status_from_usize_panics_at_upper_boundary() {
        TaskStatus::from_usize(TASK_STATUS_COUNT);
    }

    #[test]
    #[should_panic(expected = "invalid task status code")]
    fn task_status_from_usize_panics_on_invalid_large_code() {
        TaskStatus::from_usize(usize::MAX);
    }

    /// [`TASK_STATUS_COUNT`] must stay aligned with `#[repr(usize)]`
    /// discriminants.
    #[test]
    fn task_status_variant_count_matches_constant() {
        assert_eq!(
            TaskStatus::Dropped as usize + 1,
            TASK_STATUS_COUNT,
            "last discriminant + 1 must equal TASK_STATUS_COUNT"
        );
    }
}
