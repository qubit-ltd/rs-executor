// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
/// Summary returned by an immediate executor-service stop request.
///
/// The report is intentionally count-based. In a generic Rust executor service,
/// pending tasks may have different result and error types, so returning a
/// strongly typed list of unstarted tasks is not generally meaningful.
///
/// The counters are implementation-specific observations. They need not
/// share a single sampling instant. See the concrete service's
/// [`super::ExecutorService::stop`](super::ExecutorService::stop) documentation
/// for counting boundaries and whether cancellation counts represent requests
/// or successful pre-start cancellation. These counters do not establish an
/// individual task's final outcome or completion of resource cleanup.
///
/// # Examples
///
/// ```
/// use qubit_executor::service::StopReport;
///
/// let report = StopReport::new(1, 2, 1);
/// assert_eq!(report.queued, 1);
/// assert_eq!(report.running, 2);
/// assert_eq!(report.cancelled, 1);
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StopReport {
    /// Queued work reported by the implementation; see the concrete service
    /// for its sampling or removal boundary.
    pub queued: usize,

    /// Running work observed by the implementation. Implementations may count
    /// at a worker-claim boundary, so this need not match each task handle's
    /// `TaskStatus` exactly or establish that its callable has started.
    pub running: usize,

    /// Cancellations or aborts reported by the implementation; the concrete
    /// service defines whether this counts requests or successful cancellation.
    pub cancelled: usize,
}

impl StopReport {
    /// Creates a new stop report from explicit counters.
    ///
    /// # Parameters
    ///
    /// * `queued` - Queued work reported by the implementation; see the
    ///   concrete service for its sampling or removal boundary.
    /// * `running` - Running work observed by the implementation; see the
    ///   concrete service for its sampling boundary.
    /// * `cancelled` - Cancellations or aborts reported by the implementation;
    ///   the concrete service defines whether this counts requests or
    ///   successful cancellation.
    ///
    /// # Returns
    ///
    /// A report containing the supplied counters.
    #[must_use]
    #[inline]
    pub const fn new(queued: usize, running: usize, cancelled: usize) -> Self {
        Self {
            queued,
            running,
            cancelled,
        }
    }
}
