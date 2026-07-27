// =============================================================================
// qubit-style: allow source-test-pair
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_function::Callable;

use crate::task::spi::{RunningTaskSlot, TaskRunner, TaskSlot};

use super::scheduled_task_entry::{ScheduledTaskEntry, StartedScheduledTask};

/// Callable task paired with a standard task completion endpoint.
pub(crate) struct CompletableScheduledTask<R, E> {
    /// Callable to run after the scheduled instant.
    task: Box<dyn FnOnce(RunningTaskSlot<R, E>) + Send + 'static>,
    /// Runner-side completion endpoint.
    slot: TaskSlot<R, E>,
}

impl<R, E> CompletableScheduledTask<R, E> {
    /// Creates a scheduled task entry.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to run after the scheduled instant.
    /// * `slot` - Runner-side task completion endpoint.
    ///
    /// # Returns
    ///
    /// A type-erased schedulable task entry.
    pub(crate) fn new<C>(task: C, slot: TaskSlot<R, E>) -> Self
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        Self {
            task: Box::new(move |running_slot| {
                TaskRunner::new(task).run_started(running_slot);
            }),
            slot,
        }
    }
}

impl<R, E> ScheduledTaskEntry for CompletableScheduledTask<R, E>
where
    R: Send + 'static,
    E: Send + 'static,
{
    /// Marks this task as accepted.
    #[inline]
    fn accept(&self) {
        self.slot.accept();
    }

    /// Starts this task and returns a closure that completes it.
    fn start(self: Box<Self>) -> Option<StartedScheduledTask> {
        let Self { task, slot } = *self;
        match slot.try_start() {
            Ok(running_slot) => Some(Box::new(move || {
                task(running_slot);
            })),
            Err(_) => None,
        }
    }

    /// Publishes cancellation for this unstarted scheduled task.
    #[inline]
    fn cancel(self: Box<Self>) -> bool {
        let Self { slot, .. } = *self;
        slot.cancel_unstarted()
    }
}
