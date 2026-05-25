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

use qubit_function::Callable;

use crate::{
    TrackedTask,
    hook::TaskHook,
    service::SubmissionError,
    task::spi::TaskEndpointPair,
};

use super::Executor;

/// Executes tasks immediately on the caller thread.
///
/// This executor is useful for deterministic tests and simple composition
/// where task execution should happen in the same call stack.
#[derive(Clone)]
pub struct DirectExecutor {
    /// Hook notified about accepted task lifecycle events.
    hook: Option<Arc<dyn TaskHook>>,
}

impl DirectExecutor {
    /// Creates a direct executor without lifecycle hooks.
    ///
    /// # Returns
    ///
    /// A direct executor.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a copy of this executor using the supplied task hook.
    ///
    /// # Parameters
    ///
    /// * `hook` - Hook notified about accepted task lifecycle events.
    ///
    /// # Returns
    ///
    /// This executor configured with `hook`.
    #[inline]
    pub fn with_hook(mut self, hook: Arc<dyn TaskHook>) -> Self {
        self.hook = Some(hook);
        self
    }
}

impl Default for DirectExecutor {
    /// Creates a direct executor without lifecycle hooks.
    #[inline]
    fn default() -> Self {
        Self { hook: None }
    }
}

impl Executor for DirectExecutor {
    /// Executes the callable inline and returns an already completed handle.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to run on the caller thread.
    ///
    /// # Returns
    ///
    /// An already completed tracked task carrying the callable result.
    #[inline]
    fn call<C, R, E>(&self, task: C) -> Result<TrackedTask<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let (handle, slot) = TaskEndpointPair::with_optional_hook(self.hook.clone()).into_tracked_parts();
        handle.accept();
        slot.run(task);
        Ok(handle)
    }
}
