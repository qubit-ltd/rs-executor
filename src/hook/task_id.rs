// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_atomic::atomic::primitive::AtomicU64;

/// Unique identifier assigned to an accepted task.
///
/// # Examples
///
/// ```
/// use qubit_executor::hook::TaskId;
///
/// let task_id = TaskId::new(42);
/// assert_eq!(task_id.get(), 42);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(
    /// Raw numeric value used to distinguish the task.
    u64,
);

impl TaskId {
    /// Creates a task id from its raw numeric value.
    ///
    /// # Parameters
    ///
    /// * `value` - Raw task id value.
    ///
    /// # Returns
    ///
    /// A task id wrapping `value`.
    #[must_use]
    #[inline]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw numeric task id value.
    ///
    /// # Returns
    ///
    /// The raw task id value.
    #[must_use]
    #[inline]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stores the next task identifier to allocate within this process.
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

/// Allocates a fresh task id.
///
/// # Returns
///
/// A task id unique within this process until the counter wraps.
#[inline]
pub(crate) fn next_task_id() -> TaskId {
    TaskId(NEXT_TASK_ID.fetch_inc())
}
