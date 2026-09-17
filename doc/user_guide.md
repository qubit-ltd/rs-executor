# qubit-executor User Guide

[简体中文](user_guide.zh_CN.md) · [README](../README.md) · [API documentation](https://docs.rs/qubit-executor)

## Purpose and Audience

This guide targets Rust library and application authors using `qubit-executor` 0.8. It explains how to choose between a one-shot `Executor` and a lifecycle-managed `ExecutorService`, observe task results, schedule work, and shut services down without assuming that dropping a handle waits for worker resources.

The crate is an execution abstraction. It does not select an async runtime, provide a bounded thread pool, or forcibly interrupt arbitrary Rust code.

## Conceptual Model

`Executor` describes how one accepted task runs. Its `call` and `execute` methods return a `TrackedTask`, and the outer `Result` reports submission failure only.

`ExecutorService` manages admission and lifecycle. An accepted `submit_callable` returns a task-result handle; `Ok(handle)` means acceptance, not successful completion. A service moves through these states:

| State | Meaning |
| --- | --- |
| `Running` | New submissions may be accepted. |
| `ShuttingDown` | `shutdown()` rejects new work and lets accepted work finish. |
| `Stopping` | `stop()` rejects new work and requests best-effort cancellation or abort. |
| `Terminated` | No accepted work remains active after shutdown or stop. |

`TaskHandle` exposes `get`, `try_get`, `is_done`, and by-value async waiting. `TrackedTask` additionally exposes `status`, `task_id`, and pre-start `cancel`. A final result is represented by `TaskResult` and may be success, `Failed(E)`, `Panicked`, `Cancelled`, or `Dropped`.

## Scenario: Accept Work Without Choosing an Application Runtime

A library wants to run a fallible calculation and return its result, while an application decides whether execution is inline or on a separate thread. The success criterion is that the library can use the same `Executor` API and the caller can observe `42` through the returned handle.

For deterministic behavior, use `DirectExecutor`:

```rust
use std::io;

use qubit_executor::{DirectExecutor, Executor};

let executor = DirectExecutor::new();
let handle = executor.call(|| Ok::<usize, io::Error>(40 + 2))?;
assert_eq!(handle.get()?, 42);
# Ok::<(), Box<dyn std::error::Error>>(())
```

When the application needs the task to run on a separate OS thread, replace the implementation without changing the task shape:

```rust
use std::io;

use qubit_executor::{Executor, ThreadPerTaskExecutor};

let executor = ThreadPerTaskExecutor::new();
let handle = executor.call(|| Ok::<usize, io::Error>(40 + 2))?;
assert_eq!(handle.get()?, 42);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The observable result is `42`. If submission itself fails, the outer `Result` contains `SubmissionError`; if the task is accepted but fails, panics, or is cancelled, the error is returned by the task handle.

## Installation and Minimal Configuration

Add the crate to `Cargo.toml`:

```toml
qubit-executor = "0.8"
```

The package declares Rust 1.94 as its minimum Rust version. Import the `Executor` trait before calling `call` or `execute`, and import `ExecutorService` before using service methods supplied by the trait.

## Core Workflow: Managed Work

Use `ThreadPerTaskExecutorService` when a basic managed service is sufficient:

```rust
use std::io;

use qubit_executor::{ExecutorService, ThreadPerTaskExecutorService};

let service = ThreadPerTaskExecutorService::new();
let handle = service.submit_callable(|| Ok::<usize, io::Error>(40 + 2))?;
assert_eq!(handle.get()?, 42);

service.shutdown();
assert!(service.wait_termination_timeout(std::time::Duration::from_secs(1)));
# Ok::<(), Box<dyn std::error::Error>>(())
```

Call `shutdown()` when accepted work should drain. Call `stop()` when queued or unstarted work should be cancelled where possible. The returned `StopReport` exposes `queued`, `running`, and `cancelled` counts. Stopping is best effort and cannot forcibly interrupt already-running arbitrary Rust code, blocking calls, or OS-thread work. Use `wait_termination()` when the caller must wait until no accepted work remains active.

## Advanced Usage

### Delayed submission

`SingleThreadScheduledExecutorService` owns one scheduler thread and accepts a `Duration` delay or an `Instant` deadline:

```rust
use std::io;
use std::time::Duration;

use qubit_executor::{
    ExecutorService, ScheduledExecutorService, SingleThreadScheduledExecutorService,
};

let service = SingleThreadScheduledExecutorService::new("app-scheduler")?;
let handle = service.schedule_callable(Duration::from_millis(25), || {
    Ok::<usize, io::Error>(40 + 2)
})?;
assert_eq!(handle.get()?, 42);
service.shutdown();
service.wait_termination();
# Ok::<(), Box<dyn std::error::Error>>(())
```

Keep scheduled task bodies short. This implementation runs due tasks on its single scheduler thread; hand heavier work to another service when appropriate.

### Tracking, cancellation, and hooks

Use `submit_tracked_callable` when status and pre-start cancellation are needed. `cancel()` is not a guarantee that a task already running will stop; inspect the returned `CancelResult` and final `TaskResult`.

Implement `TaskHook` to observe `on_accepted`, `on_rejected`, `on_started`, and `on_finished`. Rejected submissions receive only `on_rejected` and no task id. An accepted task receives `on_accepted` before later events; a task cancelled before starting may receive `on_finished` without `on_started`. Hook panics are contained by the executor, but hooks should still be fast and non-blocking.

## Errors and Diagnostics

Submission errors are returned before a task is accepted:

- `SubmissionError::Shutdown` means shutdown or stop has closed admission.
- `SubmissionError::Saturated` indicates that a concrete service cannot accept more work.
- `SubmissionError::InvalidDeadline` means a scheduled delay cannot be represented by the target platform's `Instant` range.
- `SubmissionError::WorkerSpawnFailed` contains the worker creation error.

After acceptance, inspect the handle rather than the submission result. `TaskExecutionError::Failed(E)` is the task's own error; `Panicked`, `Cancelled`, and `Dropped` describe execution or completion conditions. Use `lifecycle()`, `is_running()`, `is_shutting_down()`, `is_stopping()`, and `is_terminated()` to diagnose service state.

## Troubleshooting

- **A submission is rejected:** inspect `lifecycle()` and the returned `SubmissionError`; do not submit after `shutdown()` or `stop()`.
- **A result is not ready:** `get()` blocks. Use `try_get()` for polling or `is_done()` for a completion check; avoid waiting on the same service thread that must run the task.
- **Termination does not return:** locate accepted tasks that are still running, especially blocking calls or OS-thread work that cannot be interrupted.
- **A scheduled task is rejected:** reduce the delay or use an `Instant` in the target platform's representable range.
- **A worker cannot start:** inspect the inner error in `WorkerSpawnFailed` and the configured thread name or stack size on the builder.

## Limitations and Best Practices

`ThreadPerTaskExecutor` and `ThreadPerTaskExecutorService` create an OS thread for each accepted task, so applications should impose their own concurrency limit. `SingleThreadScheduledExecutorService` has one scheduler thread and is not a replacement for a bounded worker pool. Neither `stop()` nor dropping a service handle guarantees immediate interruption or resource release. For deterministic cleanup, request `shutdown()` or `stop()`, wait for `wait_termination()`, and then drop handles.

## Further Reading

- [README](../README.md)
- [简体中文用户手册](user_guide.zh_CN.md)
- [API documentation](https://docs.rs/qubit-executor)
- [Crates.io](https://crates.io/crates/qubit-executor)
