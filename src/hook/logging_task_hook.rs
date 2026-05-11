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
    hook::{TaskHook, TaskId},
    service::SubmissionError,
};

/// Task hook that writes lifecycle events through the `log` facade.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LoggingTaskHook;

impl TaskHook for LoggingTaskHook {
    /// Logs task acceptance.
    #[inline]
    fn on_accepted(&self, task_id: TaskId) {
        log::debug!("task {} accepted", task_id.get());
    }

    /// Logs task rejection.
    #[inline]
    fn on_rejected(&self, error: &SubmissionError) {
        log::debug!("task rejected: {error}");
    }

    /// Logs task start.
    #[inline]
    fn on_started(&self, task_id: TaskId) {
        log::debug!("task {} started", task_id.get());
    }

    /// Logs task completion.
    #[inline]
    fn on_finished(&self, task_id: TaskId, status: TaskStatus) {
        log::debug!("task {} finished with status {:?}", task_id.get(), status);
    }
}
