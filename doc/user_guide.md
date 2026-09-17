# qubit-executor User Guide

## Scenario

Use the crate when a library accepts work without choosing a runtime. Select a
concrete service in the application: a thread pool for bounded parallelism,
the single thread scheduler for delayed work, or a Tokio or Rayon integration.

## Install

```toml
qubit-executor = "0.8"
```

Import `Executor` for one-shot calls and `ExecutorService` for lifecycle-managed
services. Returned handles expose `get`, `wait`, and cancellation operations.

## Lifecycle

Create a service, submit work, then call `shutdown` for graceful completion or
`stop` to cancel queued work. Call `wait_termination` (or its timeout variant)
before process exit. A task hook can observe accepted, rejected, and finished
events; hooks should be fast and must not depend on the task they observe.

## Scheduling and errors

`SingleThreadScheduledExecutorService` accepts a `Duration` delay. Delays that
cannot be represented by the platform clock are rejected with
`SubmissionError::InvalidDeadline`. A shutdown service returns
`SubmissionError::Shutdown`; worker creation failures return
`SubmissionError::WorkerSpawnFailed`.

## Operational limits

Thread-per-task services create one OS thread per accepted task and therefore
need an application-level concurrency limit. Scheduled services use one worker
thread. Timeout waits are monotonic and support long durations without panics,
but a timeout still returns `false` when termination has not completed.

## Troubleshooting

- If submissions are rejected, inspect `lifecycle()` and stop submitting after
  shutdown begins.
- If termination waits, check for running tasks that do not return.
- If a deadline is rejected, reduce the delay to a value representable by
  `Instant` on the target platform.

See the API documentation on [docs.rs](https://docs.rs/qubit-executor).
