/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use crate::service::ExecutorBuildError;

use super::ThreadPerTaskExecutor;

/// Builder for [`ThreadPerTaskExecutor`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ThreadPerTaskExecutorBuilder {
    /// Optional stack size for each spawned worker thread.
    stack_size: Option<usize>,
}

impl ThreadPerTaskExecutorBuilder {
    /// Creates a builder with default worker thread options.
    ///
    /// # Returns
    ///
    /// A builder that uses the platform default worker stack size.
    #[inline]
    pub const fn new() -> Self {
        Self { stack_size: None }
    }

    /// Sets the worker thread stack size.
    ///
    /// # Parameters
    ///
    /// * `stack_size` - Stack size in bytes for each worker thread.
    ///
    /// # Returns
    ///
    /// This builder with the supplied stack size.
    #[inline]
    pub const fn stack_size(mut self, stack_size: usize) -> Self {
        self.stack_size = Some(stack_size);
        self
    }

    /// Builds the executor.
    ///
    /// # Returns
    ///
    /// A thread-per-task executor with the configured worker options.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutorBuildError::ZeroStackSize`] if the configured stack
    /// size is zero.
    #[inline]
    pub fn build(self) -> Result<ThreadPerTaskExecutor, ExecutorBuildError> {
        if self.stack_size == Some(0) {
            return Err(ExecutorBuildError::ZeroStackSize);
        }
        Ok(ThreadPerTaskExecutor {
            stack_size: self.stack_size,
        })
    }
}
