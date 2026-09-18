# Qubit Executor

[![Rust CI](https://github.com/qubit-ltd/rs-executor/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-executor/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-executor/coverage-badge.json)](https://qubit-ltd.github.io/rs-executor/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-executor.svg?color=blue)](https://crates.io/crates/qubit-executor)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Qubit Executor provides runtime-neutral Rust abstractions for submitting work, observing task results, and controlling the lifecycle of managed execution services. It is for library authors who need to accept or expose executor behavior without forcing users to depend on Tokio, Rayon, or a particular thread pool.

## Quick Start

Suppose a library must submit a fallible calculation and wait for its result, while the application chooses how the task runs. Select `DirectExecutor` for deterministic same-thread execution:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
use std::io;

use qubit_executor::{DirectExecutor, Executor};

let executor = DirectExecutor::new();
let handle = executor.call(|| Ok::<usize, io::Error>(40 + 2))?;
assert_eq!(handle.get()?, 42);
Ok(())
}
```

For accepted work that must be shut down explicitly, use a managed service:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
use std::io;

use qubit_executor::{ExecutorService, ThreadPerTaskExecutorService};

let service = ThreadPerTaskExecutorService::new();
let handle = service.submit_callable(|| Ok::<usize, io::Error>(40 + 2))?;
assert_eq!(handle.get()?, 42);
service.shutdown();
service.wait_termination();
Ok(())
}
```

`submit_callable` returning `Ok(handle)` means only that the service accepted the task. After `shutdown()` or `stop()`, new submissions are rejected; call `wait_termination()` when the service must quiesce before cleanup.

## Why This Project Exists

Execution strategy, task completion, and managed-service lifecycle have different consumers. A reusable library often needs the first two without choosing an application runtime, while an application also needs admission control, shutdown, cancellation, and termination observation. Qubit Executor keeps these boundaries explicit so libraries can depend on stable task semantics and applications can select a concrete implementation.

## What It Provides

- `Executor` for one-shot fallible tasks, with `DirectExecutor`, `DelayExecutor`, `ScheduleExecutor`, and `ThreadPerTaskExecutor`.
- `ExecutorService` for accepted-task tracking, graceful `shutdown()`, best-effort `stop()`, lifecycle inspection, and blocking termination waiting.
- `ThreadPerTaskExecutorService` plus `ScheduledExecutorService` and `SingleThreadScheduledExecutorService` for delayed or `Instant`-based submission.
- `TaskHandle`, `TrackedTask`, `TaskExecutionError`, and `TaskResult` for completion, status, cancellation, panic, failure, and dropped-endpoint semantics.
- `TaskHook`, `ExecutorServiceLifecycle`, `SubmissionError`, and `StopReport` for observation and lifecycle reporting.

The crate does not provide a bounded thread pool, an async runtime, or forced interruption of arbitrary Rust code. Use `qubit-thread-pool`, `qubit-tokio-executor`, `qubit-rayon-executor`, or `qubit-execution-services` when an application needs those concrete integrations.

## Installation

Add the published crate to `Cargo.toml`:

```toml
qubit-executor = "0.8"
```

The crate requires Rust 1.94 or newer.

## Learn More

- [English user guide](doc/user_guide.md)
- [简体中文用户手册](doc/user_guide.zh_CN.md)
- [API documentation](https://docs.rs/qubit-executor)
- [简体中文 README](README.zh_CN.md)

## Testing

```bash
# Run tests with the default feature set
cargo test

# Run tests with all declared features
cargo test --all-features

# Project CI checks
./ci-check.sh

# Check code coverage
./coverage.sh
```

## License

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the
full license text.

## Contributing

Contributions are welcome. Please follow the Rust API guidelines, keep public
API documentation and tests current, and run `./align-ci.sh` to format code and
`./ci-check.sh` to satisfy CI requirements before submitting a pull request.

## Author

**Haixing Hu** - *Qubit Co. Ltd.*

Repository: [https://github.com/qubit-ltd/rs-executor](https://github.com/qubit-ltd/rs-executor)
