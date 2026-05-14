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
    Arc,
    atomic::{
        AtomicBool,
        Ordering,
    },
};

use qubit_function::Callable;

use crate::task::spi::{
    RunningTaskSlot,
    TaskRunner,
    TaskSlot,
};

use super::scheduled_task_entry::{
    ScheduledTaskEntry,
    StartedScheduledTask,
};

/// Callable task paired with a standard task completion endpoint.
pub(crate) struct CompletableScheduledTask<R, E> {
    /// Callable to run after the scheduled instant.
    task: Box<dyn FnOnce(RunningTaskSlot<R, E>) + Send + 'static>,
    /// Runner-side completion endpoint.
    slot: TaskSlot<R, E>,
    /// Shared marker used by the heap to skip externally cancelled tasks.
    cancelled: Arc<AtomicBool>,
}

impl<R, E> CompletableScheduledTask<R, E> {
    /// Creates a scheduled task entry.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to run after the scheduled instant.
    /// * `slot` - Runner-side task completion endpoint.
    /// * `cancelled` - Shared cancellation marker.
    ///
    /// # Returns
    ///
    /// A type-erased schedulable task entry.
    pub(crate) fn new<C>(task: C, slot: TaskSlot<R, E>, cancelled: Arc<AtomicBool>) -> Self
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
            cancelled,
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

    /// Returns whether this task has already been cancelled before start.
    #[inline]
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Starts this task and returns a closure that completes it.
    fn start(self: Box<Self>) -> Option<StartedScheduledTask> {
        let Self {
            task,
            slot,
            cancelled,
        } = *self;
        match slot.try_start() {
            Ok(running_slot) => Some(Box::new(move || {
                task(running_slot);
            })),
            Err(_) => {
                cancelled.store(true, Ordering::Release);
                None
            }
        }
    }

    /// Publishes cancellation for this unstarted scheduled task.
    #[inline]
    fn cancel(self: Box<Self>) -> bool {
        let Self {
            slot, cancelled, ..
        } = *self;
        if slot.cancel_unstarted() {
            cancelled.store(true, Ordering::Release);
            true
        } else {
            false
        }
    }
}
