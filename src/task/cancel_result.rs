/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
/// Result of an attempt to cancel a tracked task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelResult {
    /// The task was cancelled before it started.
    Cancelled,
    /// The task had already started and cannot be cancelled cooperatively.
    AlreadyRunning,
    /// The task had already reached a terminal state.
    AlreadyFinished,
    /// The backing service or handle does not support active cancellation.
    Unsupported,
}
