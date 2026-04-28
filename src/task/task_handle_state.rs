/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
use std::task::Waker;

use super::super::TaskResult;

/// Mutable completion state protected by the task handle mutex.
pub(crate) struct TaskHandleState<R, E> {
    /// Final task result, present only after completion and before retrieval.
    pub(crate) result: Option<TaskResult<R, E>>,
    /// Whether a runner has started executing the task.
    pub(crate) started: bool,
    /// Whether a terminal result has been published.
    pub(crate) completed: bool,
    /// Last async waker registered by polling the handle before completion.
    pub(crate) waker: Option<Waker>,
}
