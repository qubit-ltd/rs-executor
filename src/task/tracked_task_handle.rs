// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::{
    cancel_result::CancelResult, task_result_handle::TaskResultHandle, task_status::TaskStatus,
};

/// Extension interface for handles that expose active task tracking.
pub trait TrackedTaskHandle<R, E>: TaskResultHandle<R, E> {
    /// Returns the currently observed task status.
    ///
    /// # Returns
    ///
    /// The current pending, running, or terminal task status.
    fn status(&self) -> TaskStatus;

    /// Attempts to cancel the task before it starts.
    ///
    /// # Returns
    ///
    /// A precise cancellation result describing whether cancellation won.
    fn cancel(&self) -> CancelResult;
}
