# Qubit Executor

[![Rust CI](https://github.com/qubit-ltd/rs-executor/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-executor/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-executor/coverage-badge.json)](https://qubit-ltd.github.io/rs-executor/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-executor.svg?color=blue)](https://crates.io/crates/qubit-executor)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

Qubit Executor 为 Rust 提供与运行时无关的任务提交、结果观察和托管执行服务生命周期控制能力。它适合需要接收或暴露执行器能力、但不希望强制用户依赖 Tokio、Rayon 或某个具体线程池的库作者。

## 快速开始

假设一个库需要提交可失败的计算并等待结果，同时把具体的执行方式交给应用选择。需要确定性的同线程执行时选择 `DirectExecutor`：

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

如果已接受的任务需要显式收尾，则使用托管服务：

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

`submit_callable` 返回 `Ok(handle)` 只表示服务已经接受任务，不能说明任务已经开始或执行成功。调用 `shutdown()` 或 `stop()` 后，新的提交会被拒绝；如果清理前必须确认服务已经静止，应调用 `wait_termination()`。

## 为什么需要这个项目

执行策略、任务完成结果和托管服务生命周期彼此相关，但服务对象不同。可复用库通常只需要前两者，不应替应用选择运行时；应用则还需要控制接收、关闭、取消和终止状态。Qubit Executor 将这些边界明确分开，让库依赖稳定的任务语义，同时由应用选择合适的具体实现。

## 核心能力与边界

- `Executor` 用于执行一次性的可失败任务，并提供 `DirectExecutor`、`DelayExecutor`、`ScheduleExecutor` 和 `ThreadPerTaskExecutor`。
- `ExecutorService` 用于跟踪已接受任务，支持优雅关闭 `shutdown()`、尽力停止 `stop()`、生命周期查询和阻塞式终止等待。
- `ThreadPerTaskExecutorService` 提供基础托管服务；`ScheduledExecutorService` 与 `SingleThreadScheduledExecutorService` 支持延迟或基于 `Instant` 的任务提交。
- `TaskHandle`、`TrackedTask`、`TaskExecutionError` 和 `TaskResult` 统一表示完成、状态、取消、panic、失败和完成端点丢失。
- `TaskHook`、`ExecutorServiceLifecycle`、`SubmissionError` 和 `StopReport` 用于事件观察和生命周期报告。

本 crate 不提供有界线程池、异步运行时，也不能强制中断任意 Rust 代码。应用需要这些具体集成时，可使用 `qubit-thread-pool`、`qubit-tokio-executor`、`qubit-rayon-executor` 或 `qubit-execution-services`。

## 安装

在 `Cargo.toml` 中加入已发布的 crate：

```toml
qubit-executor = "0.8"
```

本 crate 要求 Rust 1.94 或更高版本。

## 延伸阅读

- [English user guide](doc/user_guide.md)
- [简体中文用户手册](doc/user_guide.zh_CN.md)
- [API 文档](https://docs.rs/qubit-executor)
- [English README](README.md)

## 测试

```bash
# 使用默认 feature 集运行测试
cargo test

# 使用项目声明的全部 feature 运行测试
cargo test --all-features

# 运行项目 CI 检查
./ci-check.sh

# 检查代码覆盖率
./coverage.sh
```

## 许可证

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

本项目基于 Apache License 2.0 授权。完整许可证文本请参阅
[LICENSE](LICENSE)。

## 贡献

欢迎贡献。请遵循 Rust API 指南，及时更新公共 API 文档与测试，并在提交
Pull Request 前运行 `./align-ci.sh`格式化代码，运行`./ci-check.sh`对齐 CI 要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-executor](https://github.com/qubit-ltd/rs-executor)
