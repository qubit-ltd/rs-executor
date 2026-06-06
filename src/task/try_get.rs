// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::TaskResult;

/// Result of a non-blocking attempt to retrieve a task result.
pub enum TryGet<H, R, E> {
    /// The task result is ready.
    Ready(TaskResult<R, E>),
    /// The task has not completed and the handle is returned to the caller.
    Pending(H),
}
