// =============================================================================
// qubit-style: allow source-test-pair
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::{
    ExecutorServiceBuilderError,
    ThreadPerTaskExecutorService,
};
use crate::hook::TaskHook;
use std::sync::Arc;

/// Builder for [`ThreadPerTaskExecutorService`].
#[derive(Clone)]
pub struct ThreadPerTaskExecutorServiceBuilder {
    /// Optional stack size for each spawned worker thread.
    pub(crate) stack_size: Option<usize>,
    /// Hook notified about accepted task lifecycle events.
    hook: Option<Arc<dyn TaskHook>>,
}

impl ThreadPerTaskExecutorServiceBuilder {
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
        self.hook = Some(hook);
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
    /// Returns [`ExecutorServiceBuilderError::ZeroStackSize`] if the configured
    /// stack size is zero.
    #[inline]
    pub fn build(
        self,
    ) -> Result<ThreadPerTaskExecutorService, ExecutorServiceBuilderError> {
        if self.stack_size == Some(0) {
            return Err(ExecutorServiceBuilderError::ZeroStackSize);
        }
        let mut service =
            ThreadPerTaskExecutorService::from_stack_size(self.stack_size);
        service.hook = self.hook;
        Ok(service)
    }
}

impl Default for ThreadPerTaskExecutorServiceBuilder {
    /// Creates a builder with default worker options and no hook.
    #[inline]
    fn default() -> Self {
        Self {
            stack_size: None,
            hook: None,
        }
    }
}
