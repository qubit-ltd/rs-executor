/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use crate::{
    TaskStatus,
    service::SubmissionError,
};

use super::TaskId;

/// Observes task lifecycle events emitted by executors and executor services.
pub trait TaskHook: Send + Sync + 'static {
    /// Called after a task is accepted.
    ///
    /// # Parameters
    ///
    /// * `task_id` - Identifier assigned to the accepted task.
    #[inline]
    fn on_accepted(&self, _task_id: TaskId) {}

    /// Called when a submitted task is rejected.
    ///
    /// # Parameters
    ///
    /// * `error` - Submission error explaining the rejection.
    #[inline]
    fn on_rejected(&self, _error: &SubmissionError) {}

    /// Called immediately before an accepted task starts running.
    ///
    /// # Parameters
    ///
    /// * `task_id` - Identifier assigned to the accepted task.
    #[inline]
    fn on_started(&self, _task_id: TaskId) {}

    /// Called after an accepted task reaches a terminal status.
    ///
    /// # Parameters
    ///
    /// * `task_id` - Identifier assigned to the accepted task.
    /// * `status` - Terminal status observed for the task.
    #[inline]
    fn on_finished(&self, _task_id: TaskId, _status: TaskStatus) {}
}
