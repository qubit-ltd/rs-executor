// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::sync::Arc;

use super::TaskId;
use crate::TaskStatus;
use crate::internal::discard_panic_payload;
use crate::service::SubmissionError;

/// Observes task lifecycle events emitted by executors and executor services.
///
/// Hook events follow a strict lifecycle ordering for each task. A rejected
/// submission emits only [`Self::on_rejected`] and never receives a task id. An
/// accepted submission emits [`Self::on_accepted`] before any later
/// [`Self::on_started`] or [`Self::on_finished`] event for the same task.
/// `on_started` is emitted only when the task actually begins running; a task
/// cancelled before start or abandoned by its accepted runner endpoint may emit
/// `on_finished` without `on_started`.
///
/// Executors contain hook panics so hook implementations cannot break task
/// execution or result publication.
///
/// Acceptance and terminal-state transitions are not serialized around the
/// full duration of user callbacks. A concurrent cancellation may therefore
/// emit `on_finished` after `on_accepted` has entered but before it returns.
/// Executors using this SPI with hooks should complete acceptance before
/// publishing a cancellable handle or allowing a worker to start the task.
///
/// Callbacks run synchronously on the thread publishing the event; they are not
/// forwarded to a dedicated observer thread. Depending on the executor and SPI
/// caller, a callback may run while an internal or caller-owned lock is held.
/// Keep callbacks short and non-blocking. Do not re-enter the same
/// [`TaskSlotCell`](crate::task::spi::TaskSlotCell) or wait for the current
/// task or its owning service to terminate from a callback.
///
/// # Examples
///
/// ```rust
/// use std::sync::atomic::AtomicUsize;
/// use std::sync::atomic::Ordering;
///
/// use qubit_executor::hook::TaskHook;
/// use qubit_executor::hook::TaskId;
///
/// struct RecordingHook(AtomicUsize);
///
/// impl TaskHook for RecordingHook {
///     fn on_accepted(&self, _task_id: TaskId) {
///         self.0.fetch_add(1, Ordering::Relaxed);
///     }
/// }
///
/// let hook = RecordingHook(AtomicUsize::new(0));
/// hook.on_accepted(TaskId::new(1));
/// assert_eq!(hook.0.load(Ordering::Relaxed), 1);
/// ```
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
    /// Rejected tasks do not have task ids and never emit started or finished
    /// events.
    ///
    /// # Parameters
    ///
    /// * `error` - Submission error explaining the rejection.
    #[inline]
    fn on_rejected(&self, _error: &SubmissionError) {}

    /// Called immediately before an accepted task starts running.
    ///
    /// This callback is emitted after [`Self::on_accepted`] for the same task.
    ///
    /// # Parameters
    ///
    /// * `task_id` - Identifier assigned to the accepted task.
    #[inline]
    fn on_started(&self, _task_id: TaskId) {}

    /// Called after an accepted task reaches a terminal status.
    ///
    /// This callback is emitted after [`Self::on_accepted`] for the same task.
    /// It is normally emitted after [`Self::on_started`], except for pre-start
    /// cancellation or accepted runner-endpoint abandonment.
    ///
    /// # Parameters
    ///
    /// * `task_id` - Identifier assigned to the accepted task.
    /// * `status` - Terminal status observed for the task.
    #[inline]
    fn on_finished(&self, _task_id: TaskId, _status: TaskStatus) {}
}

/// Invokes a callback without allowing its panic to unwind into task execution.
///
/// A panic is caught and logged; both the callback panic payload and any panic
/// raised by the logger are safely discarded.
///
/// # Parameters
///
/// * `callback` - Hook callback to invoke.
fn contain_hook_panic(callback: impl FnOnce()) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(callback)) {
        discard_panic_payload(payload);
        if let Err(log_payload) = catch_unwind(AssertUnwindSafe(|| {
            log::warn!("task hook panicked");
        })) {
            discard_panic_payload(log_payload);
        }
    }
}

/// Notifies a hook that a task was accepted, containing and logging any panic
/// from the callback.
///
/// # Parameters
///
/// * `hook` - Hook to notify.
/// * `task_id` - Accepted task id.
#[inline]
pub(crate) fn notify_accepted(hook: &dyn TaskHook, task_id: TaskId) {
    contain_hook_panic(|| hook.on_accepted(task_id));
}

/// Notifies a hook that a task was rejected, containing and logging any panic
/// from the callback.
///
/// # Parameters
///
/// * `hook` - Hook to notify.
/// * `error` - Submission error explaining the rejection.
#[inline]
pub(crate) fn notify_rejected(hook: &dyn TaskHook, error: &SubmissionError) {
    contain_hook_panic(|| hook.on_rejected(error));
}

/// Notifies an optional hook that a task was rejected, containing and logging
/// any panic from the callback. Does nothing when no hook is configured.
///
/// # Parameters
///
/// * `hook` - Optional hook to notify.
/// * `error` - Submission error explaining the rejection.
#[inline]
pub(crate) fn notify_rejected_optional(hook: Option<&Arc<dyn TaskHook>>, error: &SubmissionError) {
    if let Some(hook) = hook {
        notify_rejected(hook.as_ref(), error);
    }
}

/// Notifies a hook that a task started, containing and logging any panic from
/// the callback.
///
/// # Parameters
///
/// * `hook` - Hook to notify.
/// * `task_id` - Started task id.
#[inline]
pub(crate) fn notify_started(hook: &dyn TaskHook, task_id: TaskId) {
    contain_hook_panic(|| hook.on_started(task_id));
}

/// Notifies a hook that a task finished, containing and logging any panic from
/// the callback.
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
