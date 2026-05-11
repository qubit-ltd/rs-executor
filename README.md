# Qubit Executor

[![Rust CI](https://github.com/qubit-ltd/rs-executor/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-executor/actions/workflows/ci.yml)
[![Coverage Status](https://coveralls.io/repos/github/qubit-ltd/rs-executor/badge.svg?branch=main)](https://coveralls.io/github/qubit-ltd/rs-executor?branch=main)
[![Crates.io](https://img.shields.io/crates/v/qubit-executor.svg?color=blue)](https://crates.io/crates/qubit-executor)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Executor abstractions and task-result primitives for Rust.

## Overview

Qubit Executor provides the small common execution API used by the Qubit Rust
concurrency crates. It separates lightweight execution strategies from managed
executor services, and provides reusable task handles for implementations that
need to publish task success, task failure, panic, or cancellation.

This crate deliberately avoids depending on Tokio, Rayon, or a concrete thread
pool. Runtime-specific implementations live in smaller companion crates so
libraries can depend only on the abstraction level they need.

## Features

- Strategy-level `Executor` trait for executing one task and returning an implementation-specific result carrier.
- `DirectExecutor` for deterministic same-thread execution.
- `DelayExecutor` for delaying work before passing it to another executor.
- `ThreadPerTaskExecutor` for spawning one OS thread per task without queue management.
- Managed `ExecutorService` trait with `submit`, `submit_callable`, `shutdown`, `stop`, lifecycle inspection, and blocking termination waiting.
- `ThreadPerTaskExecutorService` as a basic managed service implementation.
- `TaskHandle`, `TrackedTask`, `TaskExecutionError`, and `TaskResult` for sharing task completion semantics across crates.
- Shared lifecycle, rejection, and stop report types through `ExecutorServiceLifecycle`, `SubmissionError`, and `StopReport`.

## Executor vs ExecutorService

`Executor` is a low-level execution strategy. It answers: “how should this one
task run, and what type represents the result?” A direct executor can return a
plain `Result`, while a thread-backed executor can return a `TaskHandle`.

`ExecutorService` is a managed service. It answers: “can this service accept a
task, track it, shut down, and eventually terminate?” A successful `submit`
means only that the service accepted the task. It does not mean the task has
started or completed successfully.

## ExecutorService Lifecycle

Every managed service follows the same high-level lifecycle:

| State | Meaning |
| --- | --- |
| `Running` | The service accepts new tasks and may have accepted work queued, scheduled, or running. |
| `ShuttingDown` | `shutdown()` has requested orderly shutdown. New submissions are rejected, while already accepted work is allowed to finish normally. |
| `Stopping` | `stop()` has requested abrupt stop. New submissions are rejected, and the service attempts to cancel or abort accepted work that can still be stopped. |
| `Terminated` | Shutdown or stop has been requested, and no accepted work remains active. |

`shutdown()` and `stop()` are both terminal admission decisions: after either
method is called, the service is no longer running and will not accept new
tasks again. The difference is how accepted work is treated.

`shutdown()` is graceful. It preserves accepted work and lets queued,
scheduled, or running tasks complete according to the concrete service's normal
execution rules.

`stop()` is abrupt and best effort. It requests cancellation of queued,
scheduled, or unstarted work, and may abort runtime-managed tasks when the
runtime supports aborting them. It cannot forcibly interrupt arbitrary Rust
code, blocking calls, or already-running OS threads, so termination may still
wait for such work to return. The returned `StopReport` describes the queued,
running, and cancelled work observed while handling the stop request.

`wait_termination()` blocks the current thread until either shutdown or stop has
been requested and all accepted work has completed, failed, panicked, been
cancelled, or been aborted according to the concrete service's capabilities.
Calling it while the service is still `Running` waits until another thread
requests shutdown or stop; if that never happens, it can block forever. This API
is deliberately synchronous and blocking, not an async or non-blocking wait.

## Task Results

`TaskHandle` represents the result of an accepted callable task. It supports
blocking waits through `get`, async waits by value, non-blocking `try_get`, and
completion checks through `is_done`.

`TrackedTask` adds status inspection and best-effort cancellation before the
task starts. Managed services expose tracked handles through
`submit_tracked` and `submit_tracked_callable`.

Task execution errors are represented by `TaskExecutionError`:

- `Failed(E)` means the task returned its own error value.
- `Panicked` means the task panicked while running.
- `Cancelled` means the task was cancelled before producing a value.

## Quick Start

### Direct execution

```rust
use std::io;

use qubit_executor::executor::{DirectExecutor, Executor};

let executor = DirectExecutor;
let value = executor.call(|| Ok::<usize, io::Error>(40 + 2))??;
assert_eq!(value, 42);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### One thread per task

```rust
use std::io;

use qubit_executor::executor::{Executor, ThreadPerTaskExecutor};

let executor = ThreadPerTaskExecutor::new();
let handle = executor.call(|| Ok::<usize, io::Error>(40 + 2))?;
assert_eq!(handle.get()?, 42);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Managed service

```rust
use std::io;

use qubit_executor::service::{ExecutorService, ThreadPerTaskExecutorService};

let service = ThreadPerTaskExecutorService::new();
let handle = service.submit_callable(|| Ok::<usize, io::Error>(40 + 2))?;
assert_eq!(handle.get()?, 42);
service.shutdown();
service.wait_termination();
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Crate Boundaries

Use `qubit-executor` when you are defining APIs that should accept or return
executor abstractions without committing to a runtime. Use a runtime-specific
crate when you need a concrete implementation:

- `qubit-thread-pool` provides dynamic and fixed OS-thread pools.
- `qubit-tokio-executor` provides Tokio-backed blocking and async IO services.
- `qubit-rayon-executor` provides a Rayon-backed CPU-bound service.
- `qubit-execution-services` aggregates the concrete services for application-level wiring.

## Testing

A minimal local run:

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

To mirror what continuous integration enforces, run the repository scripts from
the project root: `./align-ci.sh` brings local tooling and configuration in line
with CI, then `./ci-check.sh` runs the same checks the pipeline uses. For test
coverage, use `./coverage.sh` to generate or open reports.

## Contributing

Issues and pull requests are welcome.

- Open an issue for bug reports, design questions, or larger feature proposals when it helps align on direction.
- Keep pull requests scoped to one behavior change, fix, or documentation update when practical.
- Before submitting, run `./align-ci.sh` and then `./ci-check.sh` so your branch matches CI rules and passes the same checks as the pipeline.
- Add or update tests when you change runtime behavior, and update this README or public rustdoc when user-visible API behavior changes.

By contributing, you agree to license your contributions under the [Apache License, Version 2.0](LICENSE), the same license as this project.

## License

Copyright (c) 2026. Haixing Hu.

This project is licensed under the [Apache License, Version 2.0](LICENSE). See the `LICENSE` file in the repository for the full text.

## Author

**Haixing Hu** — Qubit Co. Ltd.

| | |
| --- | --- |
| **Repository** | [github.com/qubit-ltd/rs-executor](https://github.com/qubit-ltd/rs-executor) |
| **Documentation** | [docs.rs/qubit-executor](https://docs.rs/qubit-executor) |
| **Crate** | [crates.io/crates/qubit-executor](https://crates.io/crates/qubit-executor) |
