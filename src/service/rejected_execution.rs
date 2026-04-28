/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
use std::{
    io,
    sync::Arc,
};

use thiserror::Error;

/// Error returned when an executor service refuses to accept a task.
///
/// This error is about task acceptance only. It does not describe task
/// execution success or failure; accepted tasks report their final result
/// through the handle returned by the service.
///
/// # Author
///
/// Haixing Hu
#[derive(Debug, Clone, Error)]
pub enum RejectedExecution {
    /// The service has been shut down and no longer accepts new tasks.
    #[error("task rejected because the executor service is shut down")]
    Shutdown,

    /// The service is saturated and cannot accept more tasks.
    #[error("task rejected because the executor service is saturated")]
    Saturated,

    /// The service accepted the task conceptually but could not create the
    /// worker thread required to execute it.
    #[error("task rejected because the executor service failed to spawn a worker: {source}")]
    WorkerSpawnFailed {
        /// I/O error reported while spawning the worker.
        source: Arc<io::Error>,
    },
}

impl PartialEq for RejectedExecution {
    /// Compares rejection categories.
    ///
    /// Worker spawn failures compare equal by variant because [`io::Error`]
    /// does not provide value equality.
    ///
    /// # Parameters
    ///
    /// * `other` - Rejection value to compare with this value.
    ///
    /// # Returns
    ///
    /// `true` when both values have the same rejection category.
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Shutdown, Self::Shutdown)
                | (Self::Saturated, Self::Saturated)
                | (
                    Self::WorkerSpawnFailed { .. },
                    Self::WorkerSpawnFailed { .. }
                )
        )
    }
}

impl Eq for RejectedExecution {}
