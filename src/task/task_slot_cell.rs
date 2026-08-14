// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use parking_lot::Mutex;

use super::running_task_slot::RunningTaskSlot;
use super::task_slot::TaskSlot;

/// Shared ownership cell for one runner-side task slot.
///
/// Executors use this type when the worker path and a cancellation path race
/// to consume the same accepted task. Exactly one path can take the slot. A
/// failed start restores the slot so its terminal result remains owned by the
/// caller that later takes or cancels it.
pub struct TaskSlotCell<R, E> {
    /// The pending runner endpoint guarded against concurrent consumption.
    slot: Mutex<Option<TaskSlot<R, E>>>,
}

impl<R, E> TaskSlotCell<R, E> {
    /// Wraps one runner-side task slot in shared storage.
    ///
    /// The slot remains pending until a caller starts, takes, or cancels it.
    #[inline]
    pub fn new(slot: TaskSlot<R, E>) -> Self {
        Self {
            slot: Mutex::new(Some(slot)),
        }
    }

    /// Marks the stored task as accepted.
    ///
    /// Has no effect after another path has consumed the slot.
    #[inline]
    pub fn accept(&self) {
        if let Some(slot) = self.slot.lock().as_ref() {
            slot.accept();
        }
    }

    /// Takes the pending slot if this caller wins the ownership race.
    ///
    /// Returns `Some` exactly once, or `None` after another path consumed it.
    #[inline]
    pub fn take(&self) -> Option<TaskSlot<R, E>> {
        self.slot.lock().take()
    }

    /// Attempts to move the stored slot into the running state.
    ///
    /// Returns a running slot when this caller wins both ownership and the
    /// task-state start race. If the task state is already terminal, restores
    /// the pending slot and returns `None`.
    pub fn try_start(&self) -> Option<RunningTaskSlot<R, E>> {
        let mut slot = self.slot.lock();
        let pending = slot.take()?;
        match pending.try_start() {
            Ok(running) => Some(running),
            Err(pending) => {
                *slot = Some(pending);
                None
            }
        }
    }

    /// Cancels the stored task before it starts.
    ///
    /// Returns `true` only when this call owned the slot and moved its task
    /// from pending to cancelled.
    #[inline]
    pub fn cancel_unstarted(&self) -> bool {
        self.take().is_some_and(TaskSlot::cancel_unstarted)
    }
}
