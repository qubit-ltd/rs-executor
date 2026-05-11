/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::sync::Arc;

use crate::{
    hook::{
        NoopTaskHook,
        TaskHook,
    },
    service::ExecutorServiceBuilderError,
};

use super::ThreadPerTaskExecutor;

/// Builder for [`ThreadPerTaskExecutor`].
#[derive(Clone)]
pub struct ThreadPerTaskExecutorBuilder {
    /// Optional stack size for each spawned worker thread.
    stack_size: Option<usize>,
    /// Hook notified about accepted task lifecycle events.
    hook: Arc<dyn TaskHook>,
}

impl ThreadPerTaskExecutorBuilder {
    /// Creates a builder with default worker thread options.
    ///
    /// # Returns
    ///
    /// A builder that uses the platform default worker stack size.
    #[inline]
    pub fn new() -> Self {
        Self::default()
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
    pub fn stack_size(mut self, stack_size: usize) -> Self {
        self.stack_size = Some(stack_size);
        self
    }

    /// Sets the task lifecycle hook.
    ///
    /// # Parameters
    ///
    /// * `hook` - Hook notified about accepted task lifecycle events.
    ///
    /// # Returns
    ///
    /// This builder with the supplied hook.
    #[inline]
    pub fn hook(mut self, hook: Arc<dyn TaskHook>) -> Self {
        self.hook = hook;
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
    /// Returns [`ExecutorServiceBuilderError::ZeroStackSize`] if the configured stack
    /// size is zero.
    #[inline]
    pub fn build(self) -> Result<ThreadPerTaskExecutor, ExecutorServiceBuilderError> {
        if self.stack_size == Some(0) {
            return Err(ExecutorServiceBuilderError::ZeroStackSize);
        }
        Ok(ThreadPerTaskExecutor {
            stack_size: self.stack_size,
            hook: self.hook,
        })
    }
}

impl Default for ThreadPerTaskExecutorBuilder {
    /// Creates a builder with default worker options and no-op hook.
    #[inline]
    fn default() -> Self {
        Self {
            stack_size: None,
            hook: Arc::new(NoopTaskHook),
        }
    }
}
