/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
/// Summary returned by an immediate executor-service shutdown request.
///
/// The report is intentionally count-based. In a generic Rust executor service,
/// pending tasks may have different result and error types, so returning a
/// strongly typed list of unstarted tasks is not generally meaningful.
///
/// # Author
///
/// Haixing Hu
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownReport {
    /// Number of tasks that were still queued when shutdown was requested.
    pub queued: usize,

    /// Number of tasks that were running when shutdown was requested.
    pub running: usize,

    /// Number of tasks for which cancellation or abort was requested.
    pub cancelled: usize,
}

impl ShutdownReport {
    /// Creates a new shutdown report from explicit counters.
    ///
    /// # Parameters
    ///
    /// * `queued` - Number of queued tasks observed during shutdown.
    /// * `running` - Number of running tasks observed during shutdown.
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
