// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_function::Callable;

use super::ExecutorService;
use super::RejectedTask;

/// Optional extension for executor services that return a rejected callable
/// to the caller.
///
/// This trait is separate from [`ExecutorService`], so existing services keep
/// their existing submission contract. Implementations return `Err` only
/// when a callable was not accepted; in that case the error owns the original
/// callable and its captured values. Every `Err` returns that same callable
/// without calling, cloning, or dropping it, and the service retains no right
/// to execute it. An `Ok` result means the service accepted the callable, not
/// that it has started or completed.
///
/// # Example
///
/// ```
/// use qubit_executor::{RecoverableExecutorService, RejectedTask};
///
/// fn submit_or_recover<S, C>(service: &S, task: C)
///     -> Result<S::ResultHandle<usize, ()>, RejectedTask<C>>
/// where
///     S: RecoverableExecutorService,
///     C: qubit_function::Callable<usize, ()> + Send + 'static,
/// {
///     service.try_submit_callable(task)
/// }
/// ```
pub trait RecoverableExecutorService: ExecutorService {
    /// Attempts to submit a callable while retaining ownership on rejection.
    ///
    /// On success, the returned handle observes the accepted callable's
    /// result. On rejection, the returned [`RejectedTask`] owns the original
    /// callable, allowing the caller to retry it or choose another disposition.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to submit.
    ///
    /// # Returns
    ///
    /// A result handle when the callable is accepted, or the rejection reason
    /// and original callable when it is not accepted.
    ///
    /// # Type Parameters
    ///
    /// * `C` - Callable to submit.
    /// * `R` - Successful result produced by the callable.
    /// * `E` - Error type produced by the callable.
    fn try_submit_callable<C, R, E>(&self, task: C) -> Result<Self::ResultHandle<R, E>, RejectedTask<C>>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static;
}
