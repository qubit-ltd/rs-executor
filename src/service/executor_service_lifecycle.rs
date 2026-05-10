/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/

/// Lifecycle state for a managed executor service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExecutorServiceLifecycle {
    /// The service accepts new tasks and may have accepted work in progress.
    Running = 0,

    /// Graceful shutdown has started and accepted work is allowed to finish.
    ShuttingDown = 1,

    /// Abrupt stop has started and the service is cancelling or aborting work it can stop.
    Stopping = 2,

    /// The service no longer accepts tasks and has no accepted work in progress.
    Terminated = 3,
}

impl ExecutorServiceLifecycle {
    /// Converts a stored lifecycle discriminant back to an enum value.
    ///
    /// # Parameters
    ///
    /// * `value` - Raw lifecycle discriminant stored by an atomic state holder.
    ///
    /// # Returns
    ///
    /// The matching lifecycle value. Unknown values are treated as
    /// [`Self::Terminated`] so corrupted internal state fails closed.
    pub(crate) const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Running,
            1 => Self::ShuttingDown,
            2 => Self::Stopping,
            _ => Self::Terminated,
        }
    }
}
