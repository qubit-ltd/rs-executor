// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::TaskResult;
use super::try_get::TryGet;

/// Common interface for handles that expose a submitted task's final result.
///
/// # Type Parameters
///
/// * `R` - The value produced when the task succeeds.
/// * `E` - The error returned when the task itself fails.
///
/// # Examples
///
/// A generic function can consume any task result handle and return the
/// result using [`TaskResult`].
///
/// ```rust
/// use std::io;
///
/// use qubit_executor::DirectExecutor;
/// use qubit_executor::Executor;
/// use qubit_executor::TaskResult;
/// use qubit_executor::task::spi::TaskResultHandle;
///
/// fn consume<H: TaskResultHandle<usize, io::Error>>(handle: H) -> TaskResult<usize, io::Error> {
///     handle.get()
/// }
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let handle = DirectExecutor::new().call(|| Ok::<usize, io::Error>(42))?;
///     assert_eq!(consume(handle)?, 42);
///     Ok(())
/// }
/// ```
pub trait TaskResultHandle<R, E>: Send {
    /// Returns whether the task has installed a terminal state.
    ///
    /// # Returns
    ///
    /// `true` after the task has reached a terminal lifecycle state. Result
    /// publication to the handle may still be racing with this observation.
    #[must_use]
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
    #[must_use]
    fn try_get(self) -> TryGet<Self, R, E>
    where
        Self: Sized;
}
