// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;
use std::time::Duration;

use qubit_function::Callable;

use super::Executor;
use super::thread_spawn_config::ThreadSpawnConfig;
use crate::TrackedTask;
use crate::hook::TaskHook;
use crate::hook::notify_rejected_optional;
use crate::service::SubmissionError;
use crate::task::internal::PendingTaskWait;
use crate::task::spi::TaskEndpointPair;
use crate::task::spi::TaskSlot;
use crate::task::task_admission_gate::TaskAdmissionGate;

/// Type-erased worker executed by a delayed helper thread.
type Worker = Box<dyn FnOnce() + Send + 'static>;

fn wait_then_run<C, R, E>(worker_wait: &PendingTaskWait, delay: Duration, slot: TaskSlot<R, E>, task: C)
where
    C: Callable<R, E>,
{
    // The wait controls timing; the slot decides whether to run or clean up.
    let _ = worker_wait.wait_for(delay);
    slot.run(task);
}

/// Executor that starts each task after a fixed delay.
///
/// `DelayExecutor` is a small non-blocking submission strategy for cases where
/// one task should run later while the caller must return immediately. It
/// models delayed start, not minimum execution duration.
///
/// Each accepted task gets its own helper OS thread. The helper thread waits
/// for the configured delay and then runs the task. Cancelling a pending task
/// wakes this wait so its captured resources can be released early;
/// cancellation does not join the worker or guarantee that it has exited on
/// return. This keeps the submitting thread unblocked without requiring a
/// shared timer, queue, runtime, or scheduler worker. It is intentionally not a
/// general-purpose delayed task scheduler and does not coalesce timers across
/// tasks.
///
/// The returned [`TrackedTask`] is created immediately. Dropping the handle
/// does not cancel the helper thread; use [`TrackedTask::cancel`] before the
/// helper thread starts the task when pre-start cancellation is needed.
///
/// # Examples
///
/// ```rust
/// use std::time::Duration;
///
/// use qubit_executor::{DelayExecutor, Executor};
///
/// let executor = DelayExecutor::new(Duration::from_millis(1));
/// let task = executor
///     .call(|| Ok::<_, ()>(42))
///     .expect("delayed task should be accepted");
/// assert_eq!(task.get().expect("task should succeed"), 42);
/// ```
#[must_use]
#[derive(Clone)]
pub struct DelayExecutor {
    /// Duration to sleep before each submitted task starts.
    delay: Duration,
    /// Hook notified about accepted task lifecycle events.
    hook: Option<Arc<dyn TaskHook>>,
    /// Optional stack size for each helper thread.
    stack_size: Option<usize>,
}

impl DelayExecutor {
    /// Creates an executor that delays task start by the supplied duration.
    ///
    /// # Parameters
    ///
    /// * `delay` - Duration to wait before running each task.
    ///
    /// # Returns
    ///
    /// A delay executor using the supplied delay.
    #[inline]
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
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

    /// Returns the configured delay.
    ///
    /// # Returns
    ///
    /// The duration waited before each task starts.
    #[inline]
    pub const fn delay(&self) -> Duration {
        self.delay
    }

    /// Spawns one delayed worker thread.
    ///
    /// # Parameters
    ///
    /// * `worker` - Closure to run on the helper OS thread.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the helper was spawned.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::WorkerSpawnFailed`] if the operating system
    /// refuses to create the helper thread.
    fn spawn_worker(&self, worker: Worker) -> Result<(), SubmissionError> {
        ThreadSpawnConfig::new(self.stack_size).spawn(worker)
    }
}

impl Executor for DelayExecutor {
    /// Starts a helper thread that waits and then runs the callable.
    ///
    /// This method returns after the helper thread has been spawned; it does
    /// not wait for the configured delay or for task completion.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable to run after the configured delay.
    ///
    /// # Returns
    ///
    /// A [`TrackedTask`] for the delayed task.
    ///
    /// # Errors
    ///
    /// Returns [`SubmissionError::WorkerSpawnFailed`] if the helper thread
    /// cannot be created.
    fn call<C, R, E>(&self, task: C) -> Result<TrackedTask<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let (handle, slot) = TaskEndpointPair::with_optional_hook(self.hook.clone()).into_tracked_parts();
        let delay = self.delay;
        let pending_wait = Arc::new(PendingTaskWait::new());
        let worker_wait = Arc::clone(&pending_wait);
        let gate = TaskAdmissionGate::new(self.hook.is_some());
        let worker_gate = gate.clone();
        let hook = self.hook.clone();
        self.spawn_worker(Box::new(move || {
            worker_gate.wait();
            wait_then_run(&worker_wait, delay, slot, task);
        }))
        .inspect_err(|error| notify_rejected_optional(hook.as_ref(), error))?;
        handle.accept();
        gate.open();
        Ok(handle.with_pending_wait(pending_wait))
    }
}

#[cfg(test)]
mod cancellation_cleanup_tests {
    use std::io;
    use std::panic;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use super::wait_then_run;
    use crate::CancelResult;
    use crate::TaskExecutionError;
    use crate::task::internal::PendingTaskWait;
    use crate::task::spi::TaskEndpointPair;

    struct PanicWithSecondaryPayload(mpsc::Sender<()>);

    impl Drop for PanicWithSecondaryPayload {
        fn drop(&mut self) {
            let _ = self.0.send(());
            panic::panic_any(SecondaryPayload);
        }
    }

    struct SecondaryPayload;

    impl Drop for SecondaryPayload {
        fn drop(&mut self) {
            panic!("secondary panic payload drop");
        }
    }

    struct PanicOnDrop(mpsc::Sender<()>);

    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            let _ = self.0.send(());
            panic!("cancelled callable drop panic");
        }
    }

    #[test]
    fn test_cancelled_callable_drop_panic_is_contained() {
        let (handle, slot) = TaskEndpointPair::<usize, io::Error>::new().into_tracked_parts();
        slot.accept();
        assert_eq!(handle.cancel(), CancelResult::Cancelled);
        let worker_wait = Arc::new(PendingTaskWait::new());
        worker_wait.cancel();
        let (drop_started_tx, drop_started_rx) = mpsc::channel();
        let calls = Arc::new(AtomicUsize::new(0));
        let task_calls = Arc::clone(&calls);
        let probe = PanicOnDrop(drop_started_tx);
        let worker = thread::spawn(move || {
            wait_then_run(&worker_wait, Duration::from_secs(3600), slot, move || {
                let _keep = &probe;
                task_calls.fetch_add(1, Ordering::AcqRel);
                Ok::<usize, io::Error>(42)
            });
        });

        drop_started_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("cancelled callable cleanup should begin");
        worker.join().expect("worker should contain callable drop panic");

        assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
        assert_eq!(calls.load(Ordering::Acquire), 0);
    }

    #[test]
    fn test_cancelled_secondary_payload_panic_is_process_safe() {
        const CHILD_ENV: &str = "QUBIT_DELAY_SECONDARY_PANIC_CHILD";
        if std::env::var_os(CHILD_ENV).is_some() {
            let (handle, slot) = TaskEndpointPair::<(), io::Error>::new().into_tracked_parts();
            slot.accept();
            assert_eq!(handle.cancel(), CancelResult::Cancelled);
            let worker_wait = Arc::new(PendingTaskWait::new());
            worker_wait.cancel();
            let (stage_tx, stage_rx) = mpsc::channel();
            let probe = PanicWithSecondaryPayload(stage_tx);
            let worker = thread::spawn(move || {
                wait_then_run(&worker_wait, Duration::from_secs(3600), slot, move || {
                    let _keep = &probe;
                    Ok::<(), io::Error>(())
                });
            });

            stage_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("secondary payload cleanup should begin");
            worker.join().expect("worker should finish isolated payload cleanup");
            assert!(matches!(handle.get(), Err(crate::TaskExecutionError::Cancelled)));
            use std::io::Write;
            std::io::stdout().write_all(b"worker-cleanup-complete\n").unwrap();
            std::io::stdout().flush().unwrap();
            let mut release = [0_u8; 1];
            std::io::stdin()
                .read_exact(&mut release)
                .expect("parent should release child after worker join");
            return;
        }

        use std::io::BufRead;
        use std::io::Read;
        use std::io::Write;
        use std::process::Command;
        use std::process::Stdio;

        let mut child = Command::new(std::env::current_exe().expect("test executable path should be available"))
            .args([
                "--exact",
                "executor::delay_executor::cancellation_cleanup_tests::test_cancelled_secondary_payload_panic_is_process_safe",
                "--nocapture",
            ])
            .env(CHILD_ENV, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("child test process should spawn");
        let stdout = child.stdout.take().expect("child stdout should be piped");
        let stderr = child.stderr.take().expect("child stderr should be piped");
        let (stage_tx, stage_rx) = mpsc::channel();
        let stdout_reader = thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
                if line == "worker-cleanup-complete" {
                    let _ = stage_tx.send(());
                }
            }
        });
        let stderr_reader = thread::spawn(move || {
            let mut output = Vec::new();
            let _ = std::io::BufReader::new(stderr).read_to_end(&mut output);
            String::from_utf8_lossy(&output).into_owned()
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let mut cleanup_observed = false;
        let mut released = false;
        let status = loop {
            if let Some(status) = child.try_wait().expect("child status should be readable") {
                if !cleanup_observed && stage_rx.try_recv().is_ok() {
                    cleanup_observed = true;
                }
                break Some(status);
            }
            if !cleanup_observed && stage_rx.recv_timeout(Duration::from_millis(10)).is_ok() {
                cleanup_observed = true;
                let mut stdin = child.stdin.take().expect("child stdin should be piped");
                released = stdin.write_all(b"x").is_ok();
                drop(stdin);
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                break child.wait().ok();
            }
        };
        let _ = stdout_reader.join();
        let stderr = stderr_reader.join().unwrap_or_default();

        assert!(
            cleanup_observed,
            "child should report worker join completion: status={status:?}; stderr: {stderr}"
        );
        assert!(released, "parent should release child after worker join");
        assert!(
            status.is_some_and(|status| status.success()),
            "child should exit successfully: {status:?}; stderr: {stderr}"
        );
    }
}
