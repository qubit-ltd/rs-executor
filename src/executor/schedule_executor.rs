// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::{sync::Arc, thread, time::Instant};

use qubit_function::Callable;

use crate::{
    TrackedTask,
    hook::{TaskHook, notify_rejected_optional},
    service::SubmissionError,
    task::{spi::TaskEndpointPair, task_admission_gate::TaskAdmissionGate},
};

use super::{Executor, thread_spawn_config::ThreadSpawnConfig};

/// Executor that starts each task at a specified monotonic instant.
///
/// `ScheduleExecutor` creates the returned [`TrackedTask`] immediately, starts
/// a helper OS thread, waits on that helper thread until the configured
/// [`Instant`], and then runs the task on the helper thread. If the configured
/// instant is not in the future, the helper thread runs the task immediately.
#[derive(Clone)]
pub struct ScheduleExecutor {
    /// Monotonic instant at which each submitted task starts.
    instant: Instant,
    /// Hook notified about accepted task lifecycle events.
    hook: Option<Arc<dyn TaskHook>>,
    /// Optional stack size for each helper thread.
    stack_size: Option<usize>,
}

impl ScheduleExecutor {
    /// Creates an executor that starts tasks at the supplied monotonic instant.
    ///
    /// # Parameters
    ///
    /// * `instant` - Monotonic instant at which each submitted task starts.
    ///
    /// # Returns
    ///
    /// A schedule executor using `instant`.
    #[inline]
    pub fn at(instant: Instant) -> Self {
        Self {
            instant,
            hook: None,
            stack_size: None,
        }
    }

    /// Returns a copy of this executor using the supplied task hook.
    ///
    /// # Parameters
    ///
    /// * `hook` - Hook notified about accepted task lifecycle events.
    ///
    /// # Returns
    ///
    /// This executor configured with `hook`.
    #[inline]
    pub fn with_hook(mut self, hook: Arc<dyn TaskHook>) -> Self {
        self.hook = Some(hook);
        self
    }

    /// Returns a copy of this executor using the supplied helper thread stack
    /// size.
    ///
    /// # Parameters
    ///
    /// * `stack_size` - Stack size in bytes for each helper thread.
    ///
    /// # Returns
    ///
    /// This executor configured with `stack_size`.
    #[inline]
    pub fn with_stack_size(mut self, stack_size: usize) -> Self {
        self.stack_size = Some(stack_size);
        self
    }

    /// Returns the configured scheduled instant.
    ///
    /// # Returns
    ///
    /// The monotonic instant at which each task starts.
    #[inline]
    pub const fn instant(&self) -> Instant {
        self.instant
    }
}

impl Executor for ScheduleExecutor {
    /// Starts a helper thread that waits until the scheduled instant and then
    /// runs the callable.
    fn call<C, R, E>(&self, task: C) -> Result<TrackedTask<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let (handle, slot) =
            TaskEndpointPair::with_optional_hook(self.hook.clone()).into_tracked_parts();
        let instant = self.instant;
        let gate = TaskAdmissionGate::new(self.hook.is_some());
        let worker_gate = gate.clone();
        let hook = self.hook.clone();
        ThreadSpawnConfig::new(self.stack_size)
            .spawn(move || {
                worker_gate.wait();
                let now = Instant::now();
                if instant > now {
                    thread::sleep(instant.duration_since(now));
                }
                slot.run(task);
            })
            .inspect_err(|error| notify_rejected_optional(hook.as_ref(), error))?;
        handle.accept();
        gate.open();
        Ok(handle)
    }
}
