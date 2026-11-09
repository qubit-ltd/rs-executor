// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::error::Error;
use std::fmt;

use super::SubmissionError;

/// A callable that an executor service did not accept, together with the
/// reason for rejection.
///
/// The original callable remains owned by this value, so callers can inspect
/// the rejection and retry or otherwise handle the callable. Formatting this
/// error exposes only the rejection reason and never formats the callable or
/// its captured values.
#[must_use]
pub struct RejectedTask<T> {
    reason: SubmissionError,
    task: T,
}

impl<T> RejectedTask<T> {
    /// Creates a rejected task from its reason and original callable.
    #[inline]
    pub const fn new(reason: SubmissionError, task: T) -> Self {
        Self { reason, task }
    }

    /// Returns the reason the callable was rejected.
    #[inline]
    pub const fn reason(&self) -> &SubmissionError {
        &self.reason
    }

    /// Returns the rejection reason and the original callable.
    #[inline]
    pub fn into_parts(self) -> (SubmissionError, T) {
        (self.reason, self.task)
    }
}

impl<T> fmt::Debug for RejectedTask<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RejectedTask")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

impl<T> fmt::Display for RejectedTask<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reason.fmt(formatter)
    }
}

impl<T> Error for RejectedTask<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.reason)
    }
}
