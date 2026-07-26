// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::io;

use thiserror::Error;

use super::SubmissionError;

/// Error returned when an executor service cannot be built.
///
/// This error is shared by executor-service implementations whose construction
/// may validate worker, queue, timeout, stack, or thread-spawn configuration.
#[derive(Debug, Error)]
pub enum ExecutorServiceBuilderError {
    /// The configured fixed worker count is zero.
    #[error("executor service pool size must be greater than zero")]
    ZeroPoolSize,

    /// The configured maximum pool size is zero.
    #[error("executor service maximum pool size must be greater than zero")]
    ZeroMaximumPoolSize,

    /// The configured core pool size is greater than the maximum pool size.
    #[error(
        "executor service core pool size {core_pool_size} exceeds maximum pool size {maximum_pool_size}"
    )]
    CorePoolSizeExceedsMaximum {
        /// Configured core pool size.
        core_pool_size: usize,

        /// Configured maximum pool size.
        maximum_pool_size: usize,
    },

    /// The configured bounded queue capacity is zero.
    #[error("executor service queue capacity must be greater than zero")]
    ZeroQueueCapacity,

    /// The configured worker stack size is zero.
    #[error("executor service stack size must be greater than zero")]
    ZeroStackSize,

    /// The configured keep-alive timeout is zero.
    #[error("executor service keep-alive timeout must be greater than zero")]
    ZeroKeepAlive,

    /// A worker thread could not be spawned.
    #[error("failed to spawn executor service worker {}: {source}", format_worker_index(*index))]
    SpawnWorker {
        /// Index of the worker that failed to spawn when the builder knows it.
        index: Option<usize>,

        /// I/O error reported by [`std::thread::Builder::spawn`].
        source: io::Error,
    },
}

impl ExecutorServiceBuilderError {
    /// Converts a runtime worker-spawn rejection into a build error.
    ///
    /// # Parameters
    ///
    /// * `error` - Rejection produced while prestarting workers during build.
    ///
    /// # Returns
    ///
    /// A build error carrying equivalent failure context.
    pub fn from_submission_error(error: SubmissionError) -> Self {
        match error {
            SubmissionError::WorkerSpawnFailed { source } => Self::SpawnWorker {
                index: None,
                source: io::Error::new(source.kind(), source.to_string()),
            },
            SubmissionError::Shutdown => Self::SpawnWorker {
                index: None,
                source: io::Error::other("executor service shut down during prestart"),
            },
            SubmissionError::Saturated => Self::SpawnWorker {
                index: None,
                source: io::Error::other("executor service saturated during prestart"),
            },
        }
    }
}

/// Formats an optional worker index for display.
///
/// # Parameters
///
/// * `index` - Known worker index, if the builder has one.
///
/// # Returns
///
/// A display fragment for the worker index.
fn format_worker_index(index: Option<usize>) -> String {
    index
        .map(|index| index.to_string())
        .unwrap_or_else(|| "unknown".to_owned())
}

impl From<SubmissionError> for ExecutorServiceBuilderError {
    /// Converts rejected-execution reasons into build-time executor errors.
    ///
    /// # Parameters
    ///
    /// * `error` - Rejection reason observed during service construction.
    ///
    /// # Returns
    ///
    /// A build error that preserves equivalent failure context.
    fn from(error: SubmissionError) -> Self {
        Self::from_submission_error(error)
    }
}
