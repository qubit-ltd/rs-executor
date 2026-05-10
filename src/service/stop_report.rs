/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
/// Summary returned by an immediate executor-service stop request.
///
/// The report is intentionally count-based. In a generic Rust executor service,
/// pending tasks may have different result and error types, so returning a
/// strongly typed list of unstarted tasks is not generally meaningful.
///
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StopReport {
    /// Number of tasks that were still queued when stop was requested.
    pub queued: usize,

    /// Number of tasks that were running when stop was requested.
    pub running: usize,

    /// Number of tasks for which cancellation or abort was requested.
    pub cancelled: usize,
}

impl StopReport {
    /// Creates a new stop report from explicit counters.
    ///
    /// # Parameters
    ///
    /// * `queued` - Number of queued tasks observed during stop.
    /// * `running` - Number of running tasks observed during stop.
    /// * `cancelled` - Number of tasks cancellation was requested for.
    ///
    /// # Returns
    ///
    /// A report containing the supplied counters.
    #[inline]
    pub const fn new(queued: usize, running: usize, cancelled: usize) -> Self {
        Self {
            queued,
            running,
            cancelled,
        }
    }
}
