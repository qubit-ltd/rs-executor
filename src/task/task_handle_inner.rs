/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::sync::{
    Mutex,
    atomic::{AtomicU8, Ordering},
};

use super::{TaskResult, task_handle_state::TaskStatus};

/// Shared completion endpoint state for one submitted task.
pub(crate) struct TaskHandleInner<R, E> {
    /// Compact task status used for start, completion, and cancellation races.
    pub(crate) status: AtomicU8,
    /// Sender used once by the winner of the terminal state race.
    pub(crate) sender: Mutex<Option<oneshot::Sender<TaskResult<R, E>>>>,
}

impl<R, E> TaskHandleInner<R, E> {
    /// Creates shared completion state for a task result sender.
    ///
    /// # Parameters
    ///
    /// * `sender` - One-shot sender used to publish the terminal task result.
    ///
    /// # Returns
    ///
    /// Shared completion state initialized as pending.
    #[inline]
    pub(crate) fn new(sender: oneshot::Sender<TaskResult<R, E>>) -> Self {
        Self {
            status: AtomicU8::new(TaskStatus::Pending.as_u8()),
            sender: Mutex::new(Some(sender)),
        }
    }

    /// Returns the currently observed task status.
    ///
    /// # Returns
    ///
    /// The task status represented by the internal atomic state.
    #[inline]
    pub(crate) fn status(&self) -> TaskStatus {
        TaskStatus::from_u8(self.status.load(Ordering::Acquire))
    }

    /// Attempts to move the task from pending to running.
    ///
    /// # Returns
    ///
    /// `true` if this call started the task, or `false` if the task was already
    /// running or terminal.
    #[inline]
    pub(crate) fn start(&self) -> bool {
        self.status
            .compare_exchange(
                TaskStatus::Pending.as_u8(),
                TaskStatus::Running.as_u8(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// Attempts to publish a terminal result when the current status allows it.
    ///
    /// # Parameters
    ///
    /// * `result` - Final task result to publish.
    /// * `can_finish` - Predicate deciding whether the current status may
    ///   transition to a terminal state.
    ///
    /// # Returns
    ///
    /// `true` if this call published the terminal result, or `false` if another
    /// path already won or `can_finish` rejected the observed status.
    pub(crate) fn finish<F>(&self, result: TaskResult<R, E>, mut can_finish: F) -> bool
    where
        F: FnMut(TaskStatus) -> bool,
    {
        let next = TaskStatus::from_result(&result);
        loop {
            let current = self.status();
            if current.is_done() || !can_finish(current) {
                return false;
            }
            if self
                .status
                .compare_exchange(
                    current.as_u8(),
                    next.as_u8(),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                let sender = self
                    .sender
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take();
                if let Some(sender) = sender {
                    let _ignored = sender.send(result);
                }
                return true;
            }
        }
    }
}
