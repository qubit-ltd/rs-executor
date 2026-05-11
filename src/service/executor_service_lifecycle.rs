/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/

/// Lifecycle state for a managed executor service.
///
/// The lifecycle is an admission and termination state machine shared by
/// [`ExecutorService`](super::ExecutorService) implementations:
///
/// * [`Running`](Self::Running) accepts new tasks.
/// * [`ShuttingDown`](Self::ShuttingDown) is entered by
///   [`ExecutorService::shutdown`](super::ExecutorService::shutdown). It
///   rejects new tasks but lets already accepted work finish normally.
/// * [`Stopping`](Self::Stopping) is entered by
///   [`ExecutorService::stop`](super::ExecutorService::stop). It rejects new
///   tasks and asks the implementation to cancel or abort accepted work that
///   can still be stopped.
/// * [`Terminated`](Self::Terminated) means shutdown or stop has been requested
///   and no accepted work remains active.
///
/// `ShuttingDown` and `Stopping` are both non-running states. The distinction is
/// what happens to accepted work: orderly shutdown preserves accepted work,
/// while abrupt stop is a best-effort cancellation or abort request. Already
/// running blocking code or OS threads may not be forcibly stopped; concrete
/// services document those runtime-specific limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExecutorServiceLifecycle {
    /// The service accepts new tasks and may have accepted work in progress.
    Running = 0,

    /// Graceful shutdown has started.
    ///
    /// The service rejects new submissions, but work accepted before
    /// [`ExecutorService::shutdown`](super::ExecutorService::shutdown) is
    /// allowed to finish normally.
    ShuttingDown = 1,

    /// Abrupt stop has started.
    ///
    /// The service rejects new submissions and is cancelling or aborting
    /// accepted work it can still stop. Work that is already running in a form
    /// the runtime cannot interrupt may continue until that work returns.
    Stopping = 2,

    /// The service no longer accepts tasks and has no accepted work in progress.
    ///
    /// This state is reached only after shutdown or stop has been requested and
    /// all accepted work has completed, been cancelled, or been aborted.
    Terminated = 3,
}

impl ExecutorServiceLifecycle {
    /// Converts a stored lifecycle discriminant back to an enum value.
    ///
    /// # Parameters
    ///
    /// * `value` - Raw lifecycle discriminant stored by an atomic state holder.
    ///
    /// # Returns
    ///
    /// The matching lifecycle value. Unknown values are treated as
    /// [`Self::Terminated`] so corrupted internal state fails closed.
    pub(crate) const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Running,
            1 => Self::ShuttingDown,
            2 => Self::Stopping,
            _ => Self::Terminated,
        }
    }
}
