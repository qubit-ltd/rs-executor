/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use qubit_atomic::atomic::primitive::AtomicU64;

/// Unique identifier assigned to an accepted task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

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
    #[inline]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw numeric task id value.
    ///
    /// # Returns
    ///
    /// The raw task id value.
    #[inline]
    pub const fn get(self) -> u64 {
        self.0
    }
}

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
