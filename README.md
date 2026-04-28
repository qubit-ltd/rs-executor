# Qubit Executor

[![CircleCI](https://circleci.com/gh/qubit-ltd/rs-executor.svg?style=shield)](https://circleci.com/gh/qubit-ltd/rs-executor)
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
- `FutureExecutor` marker trait for executors whose carrier is future-like.
- `DirectExecutor` for deterministic same-thread execution.
- `DelayExecutor` for delaying work before passing it to another executor.
- `ThreadPerTaskExecutor` for spawning one OS thread per task without queue management.
- Managed `ExecutorService` trait with `submit`, `submit_callable`, `shutdown`, `shutdown_now`, and termination waiting.
- `ThreadPerTaskExecutorService` as a basic managed service implementation.
- `TaskHandle`, `TaskCompletion`, `TaskExecutionError`, and `TaskResult` for sharing task completion semantics across crates.
- Shared rejection and shutdown report types through `RejectedExecution` and `ShutdownReport`.

## Executor vs ExecutorService

`Executor` is a low-level execution strategy. It answers: “how should this one
task run, and what type represents the result?” A direct executor can return a
plain `Result`, while a thread-backed executor can return a `TaskHandle`.

`ExecutorService` is a managed service. It answers: “can this service accept a
task, track it, shut down, and eventually terminate?” A successful `submit`
means only that the service accepted the task. It does not mean the task has
started or completed successfully.

## Task Results

`TaskHandle` represents an accepted task. It supports blocking waits through
`get`, async waits through `Future`, completion checks through `is_done`, and
best-effort cancellation before the task starts.

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
let value = executor.call(|| Ok::<usize, io::Error>(40 + 2))?;
assert_eq!(value, 42);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### One thread per task

```rust
use std::io;

use qubit_executor::executor::{Executor, ThreadPerTaskExecutor};

let executor = ThreadPerTaskExecutor;
let handle = executor.call(|| Ok::<usize, io::Error>(40 + 2));
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

Copyright © 2026 Haixing Hu, Qubit Co. Ltd.

This project is licensed under the [Apache License, Version 2.0](LICENSE). See the `LICENSE` file in the repository for the full text.

## Author

**Haixing Hu** — Qubit Co. Ltd.

| | |
| --- | --- |
| **Repository** | [github.com/qubit-ltd/rs-executor](https://github.com/qubit-ltd/rs-executor) |
| **Documentation** | [docs.rs/qubit-executor](https://docs.rs/qubit-executor) |
| **Crate** | [crates.io/crates/qubit-executor](https://crates.io/crates/qubit-executor) |
