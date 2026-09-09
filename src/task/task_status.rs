// =============================================================================
// qubit-style: allow source-test-pair
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow inline-tests
use qubit_state_machine::DenseCode;

use super::TaskExecutionError;
use super::TaskResult;

/// Observable lifecycle status for a submitted task.
///
/// `#[repr(usize)]` assigns stable discriminants `0..7` for
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

impl DenseCode for TaskStatus {
    const VALUES: &'static [Self] = &[
        Self::Pending,
        Self::Running,
        Self::Succeeded,
        Self::Failed,
        Self::Panicked,
        Self::Cancelled,
        Self::Dropped,
    ];
    /// Returns the stable compact task-state code.
    #[inline]
    fn code(self) -> u64 {
        self as u64
    }
}

#[cfg(test)]
mod compact_encoding_tests {
    use qubit_state_machine::DenseCode;

    use super::TaskStatus;

    #[test]
    fn test_task_status_codebook_preserves_discriminants() {
        let expected = [
            TaskStatus::Pending,
            TaskStatus::Running,
            TaskStatus::Succeeded,
            TaskStatus::Failed,
            TaskStatus::Panicked,
            TaskStatus::Cancelled,
            TaskStatus::Dropped,
        ];
        assert_eq!(TaskStatus::VALUES, expected);
        for (index, &value) in TaskStatus::VALUES.iter().enumerate() {
            assert_eq!(value.code(), index as u64);
        }
    }
}
