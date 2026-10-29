// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use crate::hook::TaskHook;

/// Task hook that ignores all events.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use qubit_executor::DirectExecutor;
/// use qubit_executor::Executor;
/// use qubit_executor::hook::NoopTaskHook;
///
/// let executor = DirectExecutor::new().with_hook(Arc::new(NoopTaskHook));
/// let task = executor.call(|| Ok::<usize, ()>(42)).expect("task accepted");
/// assert_eq!(task.get().expect("task completed"), 42);
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NoopTaskHook;

impl TaskHook for NoopTaskHook {}
