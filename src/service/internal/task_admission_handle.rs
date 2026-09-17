// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use crate::TrackedTask;
use crate::task::TaskHandle;

/// Internal abstraction for publishing task acceptance on a result handle.
pub trait TaskAdmissionHandle {
    /// Marks the associated task as accepted and emits its acceptance hook.
    fn mark_accepted(&self);
}

impl<R, E> TaskAdmissionHandle for TaskHandle<R, E> {
    /// Marks the task handle as accepted.
    #[inline]
    fn mark_accepted(&self) {
        self.accept();
    }
}

impl<R, E> TaskAdmissionHandle for TrackedTask<R, E> {
    /// Marks the tracked task as accepted.
    #[inline]
    fn mark_accepted(&self) {
        self.accept();
    }
}
