// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::TaskResult;

/// Outcome of a non-blocking attempt to retrieve a task result.
///
/// A ready outcome contains the task's final result. A pending outcome returns
/// ownership of the handle so the caller can try again later or wait for the
/// task to finish.
///
/// # Type Parameters
///
/// * `H` - The task handle returned when the result is not ready yet.
/// * `R` - The value produced when the task succeeds.
/// * `E` - The error returned when the task itself fails.
///
/// # Examples
///
/// ```
/// use qubit_executor::TryGet;
///
/// let outcome: TryGet<(), i32, ()> = TryGet::Ready(Ok(42));
/// match outcome {
///     TryGet::Ready(Ok(value)) => assert_eq!(value, 42),
///     TryGet::Ready(Err(_)) => panic!("the task failed"),
///     TryGet::Pending(()) => panic!("the task is still pending"),
/// }
/// ```
pub enum TryGet<H, R, E> {
    /// The task result is ready.
    Ready(TaskResult<R, E>),
    /// The task has not completed and the handle is returned to the caller.
    Pending(H),
}
