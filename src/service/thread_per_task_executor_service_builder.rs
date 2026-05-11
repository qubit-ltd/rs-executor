/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use super::{
    ExecutorBuildError,
    ThreadPerTaskExecutorService,
};

/// Builder for [`ThreadPerTaskExecutorService`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ThreadPerTaskExecutorServiceBuilder {
    /// Optional stack size for each spawned worker thread.
    pub(crate) stack_size: Option<usize>,
}

impl ThreadPerTaskExecutorServiceBuilder {
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

    /// Builds the executor service.
    ///
    /// # Returns
    ///
    /// A thread-per-task executor service with configured worker options.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutorBuildError::ZeroStackSize`] if the configured stack
    /// size is zero.
    #[inline]
    pub fn build(self) -> Result<ThreadPerTaskExecutorService, ExecutorBuildError> {
        if self.stack_size == Some(0) {
            return Err(ExecutorBuildError::ZeroStackSize);
        }
        Ok(ThreadPerTaskExecutorService::from_stack_size(
            self.stack_size,
        ))
    }
}
