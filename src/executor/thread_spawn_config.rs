/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::thread;

use crate::service::SubmissionError;

/// Shared worker-thread spawn configuration.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ThreadSpawnConfig {
    /// Optional stack size for spawned worker threads.
    pub(crate) stack_size: Option<usize>,
}

impl ThreadSpawnConfig {
    /// Creates a spawn configuration from an optional stack size.
    ///
    /// # Parameters
    ///
    /// * `stack_size` - Optional stack size in bytes.
    ///
    /// # Returns
    ///
    /// A thread spawn configuration.
    #[inline]
    pub(crate) const fn new(stack_size: Option<usize>) -> Self {
        Self { stack_size }
    }

    /// Spawns one worker thread.
    ///
    /// # Parameters
    ///
    /// * `worker` - Closure to run on the new OS thread.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the worker was spawned.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::WorkerSpawnFailed`] if the operating system
    /// refuses to create the worker thread.
    pub(crate) fn spawn(self, worker: impl FnOnce() + Send + 'static) -> Result<(), SubmissionError> {
        let mut builder = thread::Builder::new();
        if let Some(stack_size) = self.stack_size {
            builder = builder.stack_size(stack_size);
        }
        builder
            .spawn(worker)
            .map(drop)
            .map_err(SubmissionError::worker_spawn_failed)
    }
}
