// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;

use crate::CancelResult;

/// A clonable endpoint for cancelling a task while it is still pending.
///
/// Clones share the same cancellation action. Dropping an endpoint does not
/// cancel the task. The action may synchronously clean up executor resources,
/// block, or panic; panics are propagated to the caller. Cancellation cannot
/// forcibly stop a task that has started running.
///
/// This endpoint can outlive the result handle and keeps resources captured by
/// its cancellation action alive until the final clone is dropped.
///
/// Executor providers can use [`TaskCancellationHandle::new`] to adapt their
/// own cancellation path. The callback must be safe for concurrent invocation
/// and must perform all executor-specific cleanup when it wins cancellation.
///
/// # Examples
///
/// Obtain the cancellation endpoint before consuming the task handle. The
/// endpoint remains usable while the result future is held:
///
/// ```
/// use std::future::IntoFuture;
///
/// use qubit_executor::task::spi::TaskEndpointPair;
/// use qubit_executor::{CancelResult, TaskExecutionError};
///
/// let (task, slot) = TaskEndpointPair::<usize, ()>::new().into_tracked_parts();
/// slot.accept();
/// let cancellation = task.cancellation_handle();
/// let result = task.into_future();
/// assert_eq!(cancellation.cancel(), CancelResult::Cancelled);
/// assert!(!slot.run(|| Ok(42)));
/// // The result future remains available to await after cancellation.
/// drop(result);
///
/// let (task, slot) = TaskEndpointPair::<usize, ()>::new().into_tracked_parts();
/// slot.accept();
/// assert_eq!(task.cancellation_handle().cancel(), CancelResult::Cancelled);
/// assert!(!slot.run(|| Ok(42)));
/// assert!(matches!(task.get(), Err(TaskExecutionError::Cancelled)));
/// ```
#[derive(Clone)]
pub struct TaskCancellationHandle {
    /// Cancellation action shared by all endpoint clones.
    cancel: Arc<dyn Fn() -> CancelResult + Send + Sync + 'static>,
}

impl TaskCancellationHandle {
    /// Creates a cancellation endpoint from a provider-supplied action.
    ///
    /// # Type Parameters
    ///
    /// * `F` - Thread-safe callback that attempts cancellation.
    ///
    /// # Parameters
    ///
    /// * `cancel` - Provider cancellation action, including executor-specific
    ///   cleanup.
    ///
    /// The callback is not run during construction or when the endpoint is
    /// dropped. It may be called concurrently through cloned endpoints, so it
    /// must safely coordinate state transitions and executor cleanup. Any
    /// panic from the callback is propagated by [`Self::cancel`].
    pub fn new<F>(cancel: F) -> Self
    where
        F: Fn() -> CancelResult + Send + Sync + 'static,
    {
        Self {
            cancel: Arc::new(cancel),
        }
    }

    /// Attempts to cancel the associated task.
    ///
    /// The callback's [`CancelResult`] reports whether cancellation won or
    /// whether the task is already running or finished.
    pub fn cancel(&self) -> CancelResult {
        (self.cancel)()
    }
}
