/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
/// Runnable closure produced after a scheduled task wins the start race.
pub(crate) type StartedScheduledTask = Box<dyn FnOnce() + Send + 'static>;

/// Type-erased scheduled task stored in the timer heap.
pub(crate) trait ScheduledTaskEntry: Send + 'static {
    /// Marks this task as accepted by the scheduled service.
    fn accept(&self);

    /// Returns whether this task has already been cancelled before start.
    fn is_cancelled(&self) -> bool;

    /// Attempts to move this task from pending to running state.
    ///
    /// # Returns
    ///
    /// A runnable closure if the task should execute, or `None` if it had already
    /// been cancelled before start.
    fn start(self: Box<Self>) -> Option<StartedScheduledTask>;

    /// Cancels this task before it starts.
    ///
    /// # Returns
    ///
    /// `true` if this call published task cancellation.
    fn cancel(self: Box<Self>) -> bool;
}
