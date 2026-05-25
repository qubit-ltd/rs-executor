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
    cmp::Ordering as CompareOrdering,
    time::Instant,
};

use super::scheduled_task_entry::ScheduledTaskEntry;

/// Task stored in the single-thread scheduled service heap.
pub(crate) struct ScheduledTask {
    /// Time at which this task becomes runnable.
    pub(crate) deadline: Instant,
    /// Insertion order used to make equal deadlines deterministic.
    sequence: usize,
    /// Type-erased accepted task.
    pub(crate) entry: Box<dyn ScheduledTaskEntry>,
}

impl ScheduledTask {
    /// Creates a heap entry for a scheduled task.
    ///
    /// # Parameters
    ///
    /// * `deadline` - Instant when the task becomes runnable.
    /// * `sequence` - Stable insertion sequence.
    /// * `entry` - Type-erased task entry.
    ///
    /// # Returns
    ///
    /// A scheduled task heap entry.
    pub(crate) const fn new(deadline: Instant, sequence: usize, entry: Box<dyn ScheduledTaskEntry>) -> Self {
        Self {
            deadline,
            sequence,
            entry,
        }
    }
}

impl Eq for ScheduledTask {}

impl PartialEq for ScheduledTask {
    /// Compares heap ordering keys.
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline && self.sequence == other.sequence
    }
}

impl Ord for ScheduledTask {
    /// Orders earliest deadlines first when used in [`std::collections::BinaryHeap`].
    #[inline]
    fn cmp(&self, other: &Self) -> CompareOrdering {
        other
            .deadline
            .cmp(&self.deadline)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

impl PartialOrd for ScheduledTask {
    /// Delegates partial ordering to the total heap ordering.
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<CompareOrdering> {
        Some(self.cmp(other))
    }
}
