// =============================================================================
// qubit-style: allow source-test-pair
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::TaskResult;
use super::try_get::TryGet;

/// Common interface for handles that expose a submitted task's final result.
pub trait TaskResultHandle<R, E>: Send {
    /// Returns whether the task has installed a terminal state.
    ///
    /// # Returns
    ///
    /// `true` after the task has reached a terminal lifecycle state. Result
    /// publication to the handle may still be racing with this observation.
    fn is_done(&self) -> bool;

    /// Blocks until the task produces its final result.
    ///
    /// # Returns
    ///
    /// The final task result. If the completion endpoint is dropped without
    /// publishing a result, a dropped-result error is reported.
    fn get(self) -> TaskResult<R, E>
    where
        Self: Sized;

    /// Attempts to retrieve the final result without blocking.
    ///
    /// # Returns
    ///
    /// [`TryGet::Ready`] when a result is available, otherwise
    /// [`TryGet::Pending`] containing the original handle.
    fn try_get(self) -> TryGet<Self, R, E>
    where
        Self: Sized;
}
