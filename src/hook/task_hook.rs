/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::panic::{
    AssertUnwindSafe,
    catch_unwind,
};

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

/// Calls a task hook and contains hook panics.
///
/// # Parameters
///
/// * `callback` - Hook callback to invoke.
fn contain_hook_panic(callback: impl FnOnce()) {
    if catch_unwind(AssertUnwindSafe(callback)).is_err() {
        log::warn!("task hook panicked");
    }
}

/// Notifies a hook that a task was accepted.
///
/// # Parameters
///
/// * `hook` - Hook to notify.
/// * `task_id` - Accepted task id.
#[inline]
pub(crate) fn notify_accepted(hook: &dyn TaskHook, task_id: TaskId) {
    contain_hook_panic(|| hook.on_accepted(task_id));
}

/// Notifies a hook that a task was rejected.
///
/// # Parameters
///
/// * `hook` - Hook to notify.
/// * `error` - Submission error explaining the rejection.
#[inline]
pub(crate) fn notify_rejected(hook: &dyn TaskHook, error: &SubmissionError) {
    contain_hook_panic(|| hook.on_rejected(error));
}

/// Notifies a hook that a task started.
///
/// # Parameters
///
/// * `hook` - Hook to notify.
/// * `task_id` - Started task id.
#[inline]
pub(crate) fn notify_started(hook: &dyn TaskHook, task_id: TaskId) {
    contain_hook_panic(|| hook.on_started(task_id));
}

/// Notifies a hook that a task finished.
///
/// # Parameters
///
/// * `hook` - Hook to notify.
/// * `task_id` - Finished task id.
/// * `status` - Terminal task status.
#[inline]
pub(crate) fn notify_finished(hook: &dyn TaskHook, task_id: TaskId, status: TaskStatus) {
    contain_hook_panic(|| hook.on_finished(task_id, status));
}
