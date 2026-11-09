// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::TaskCancellationHandle;
use super::tracked_task_handle::TrackedTaskHandle;

/// Optional extension for tracked handles that expose an independent
/// cancellation endpoint.
///
/// Implement this trait when a handle can preserve its complete executor
/// specific cancellation path in an endpoint that remains usable after the
/// handle is consumed into a future. Existing [`TrackedTaskHandle`] types do
/// not need to implement this trait unless callers need that capability.
///
/// Taking or dropping the endpoint does not cancel the task. An endpoint may
/// keep shared task or executor state alive, and its cancellation action may
/// block or panic. Cancellation is best effort and cannot interrupt arbitrary
/// code that is already running.
///
/// # Type Parameters
///
/// - `R`: The task's successful result type.
/// - `E`: The task's error type.
///
/// # Examples
///
/// ```no_run
/// use std::future::IntoFuture;
///
/// use qubit_executor::{CancellableTaskHandle, TaskResult};
///
/// async fn observe<H>(handle: H) -> TaskResult<usize, ()>
/// where
///     H: CancellableTaskHandle<usize, ()>
///         + IntoFuture<Output = TaskResult<usize, ()>>,
/// {
///     let cancellation = CancellableTaskHandle::cancellation_handle(&handle);
///     let result = handle.into_future().await;
///     drop(cancellation);
///     result
/// }
/// ```
pub trait CancellableTaskHandle<R, E>: TrackedTaskHandle<R, E> {
    /// Creates an independent endpoint for cancelling this task while pending.
    ///
    /// The endpoint must preserve all executor-specific cleanup performed by
    /// the originating handle's cancellation method.
    #[must_use]
    fn cancellation_handle(&self) -> TaskCancellationHandle;
}
