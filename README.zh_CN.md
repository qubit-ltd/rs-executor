# Qubit Executor

[![Rust CI](https://github.com/qubit-ltd/rs-executor/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-executor/actions/workflows/ci.yml)
[![Coverage Status](https://coveralls.io/repos/github/qubit-ltd/rs-executor/badge.svg?branch=main)](https://coveralls.io/github/qubit-ltd/rs-executor?branch=main)
[![Crates.io](https://img.shields.io/crates/v/qubit-executor.svg?color=blue)](https://crates.io/crates/qubit-executor)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Documentation](https://img.shields.io/badge/docs-English-blue.svg)](README.md)

面向 Rust 的 executor 抽象与任务结果原语。

## 概览

Qubit Executor 提供 Qubit Rust 并发 crate 共享的最小执行 API。它把轻量级执行策略与托管 executor service 分开，并提供可复用的任务 handle，用于发布任务成功、任务失败、panic、取消或完成端点被丢弃等状态。

本 crate 刻意不依赖 Tokio、Rayon 或具体线程池。依赖运行时的实现放在更小的配套 crate 中，便于库作者只依赖自己需要的抽象层。

## 功能

- 提供策略级 `Executor` trait，用于执行单个任务并返回 `TrackedTask` handle。
- 提供 `DirectExecutor`，用于确定性的同线程执行。
- 提供 `DelayExecutor`，用于在辅助线程等待固定时长后执行任务。
- 提供 `ScheduleExecutor`，用于在单调时钟的指定 `Instant` 执行任务。
- 提供 `ThreadPerTaskExecutor`，用于每个任务启动一个 OS 线程且不管理队列。
- 提供托管 `ExecutorService` trait，支持 `submit`、`submit_callable`、`shutdown`、`stop`、生命周期查询和阻塞式等待终止。
- 提供 `ThreadPerTaskExecutorService` 作为基础托管服务实现。
- 提供 `TaskHandle`、`TrackedTask`、`TaskExecutionError` 与 `TaskResult`，用于在多个 crate 之间共享任务完成语义。
- 通过 `ExecutorServiceLifecycle`、`SubmissionError` 与 `StopReport` 提供共享的生命周期、拒绝执行原因和停止报告类型。

## Executor 与 ExecutorService

`Executor` 是底层执行策略。它回答的是：“这个单个任务应该如何运行？”已接受任务的结果统一通过 `TrackedTask` 暴露，即使具体 executor 是同线程内联执行。

`ExecutorService` 是托管服务。它回答的是：“这个服务是否能接受任务、跟踪任务、关闭并最终终止？”`submit` 成功只表示服务接受了任务，不表示任务已经开始或成功完成。

## ExecutorService 生命周期

所有托管服务都遵循相同的高层生命周期：

| 状态 | 含义 |
| --- | --- |
| `Running` | 服务接受新任务，并且可能已有已接受的任务正在排队、等待调度或运行。 |
| `ShuttingDown` | 已调用 `shutdown()` 请求有序关闭。新提交会被拒绝，已接受的工作会被允许正常完成。 |
| `Stopping` | 已调用 `stop()` 请求立即停止。新提交会被拒绝，服务会尝试取消或 abort 仍可停止的已接受工作。 |
| `Terminated` | 已请求 shutdown 或 stop，且没有任何已接受工作仍处于活动状态。 |

`shutdown()` 和 `stop()` 都是终止性的接收开关：调用任一方法后，服务都不再处于 running 状态，也不会再次接受新任务。两者的区别在于如何处理已经接受的工作。

`shutdown()` 是优雅关闭。它保留已接受的工作，并允许排队中、等待调度中或运行中的任务按照具体服务的正常执行规则完成。

`stop()` 是立即停止，并且是尽力而为。它会请求取消排队中、等待调度中或尚未开始的工作；如果运行时支持 abort，也可以 abort 由运行时管理的任务。它不能强行中断任意 Rust 代码、blocking 调用或已经运行的 OS 线程，因此服务终止仍可能等待这些工作自行返回。返回的 `StopReport` 描述处理 stop 请求时观察到的 queued、running 与 cancelled 工作数量。

`wait_termination()` 会阻塞当前线程，直到 shutdown 或 stop 已经被请求，并且所有已接受工作都已经完成、失败、panic、被取消或按照具体服务能力被 abort。如果在服务仍处于 `Running` 状态时调用，它会一直等待另一个线程请求 shutdown 或 stop；如果这件事永远不发生，该调用可能永久阻塞。这个 API 明确是同步阻塞式等待，不是 async 或非阻塞等待。

## 任务结果

`TaskHandle` 表示已接受 callable 任务的结果。它支持通过 `get` 阻塞等待、按值 await、通过 `try_get` 非阻塞尝试获取结果，以及通过 `is_done` 检查完成状态。

`TrackedTask` 在结果获取之外增加状态检查和任务开始前的尽力取消。托管服务通过 `submit_tracked` 和 `submit_tracked_callable` 返回 tracked handle。

任务执行错误由 `TaskExecutionError` 表示：

- `Failed(E)` 表示任务返回了自己的错误值。
- `Panicked` 表示任务运行期间发生 panic。
- `Cancelled` 表示任务在产生结果前被取消。
- `Dropped` 表示 runner 侧完成端点在发布结果前消失；这与显式取消请求不同。

## 快速开始

### 直接执行

```rust
use std::io;

use qubit_executor::{DirectExecutor, Executor};

let executor = DirectExecutor::new();
let handle = executor.call(|| Ok::<usize, io::Error>(40 + 2))?;
let value = handle.get()?;
assert_eq!(value, 42);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### 每个任务一个线程

```rust
use std::io;

use qubit_executor::{Executor, ThreadPerTaskExecutor};

let executor = ThreadPerTaskExecutor::new();
let handle = executor.call(|| Ok::<usize, io::Error>(40 + 2))?;
assert_eq!(handle.get()?, 42);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### 托管服务

```rust
use std::io;

use qubit_executor::{ExecutorService, ThreadPerTaskExecutorService};

let service = ThreadPerTaskExecutorService::new();
let handle = service.submit_callable(|| Ok::<usize, io::Error>(40 + 2))?;
assert_eq!(handle.get()?, 42);
service.shutdown();
service.wait_termination();
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Crate 边界

当你在定义 API，且希望接受或返回 executor 抽象而不绑定具体运行时时，使用 `qubit-executor`。需要具体实现时，使用对应的运行时 crate：

- `qubit-thread-pool` 提供动态与固定大小的 OS 线程池。
- `qubit-tokio-executor` 提供基于 Tokio 的 blocking 与 async IO 服务。
- `qubit-rayon-executor` 提供基于 Rayon 的 CPU 密集型服务。
- `qubit-execution-services` 为应用层装配聚合后的具体服务。

## 测试

快速在本地跑一遍：

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

若要与持续集成（CI）保持一致，请在仓库根目录依次执行：`./align-ci.sh` 将本地工具链与配置对齐到 CI 规则，再执行 `./ci-check.sh` 复现流水线中的检查。需要查看或生成测试覆盖率时，使用 `./coverage.sh`。

## 参与贡献

欢迎通过 Issue 与 Pull Request 参与本仓库。建议：

- 报告缺陷、讨论设计或较大能力扩展时，可先开 Issue 对齐方向再投入实现。
- 单次 PR 尽量聚焦单一主题，便于代码审查与合并历史。
- 提交 PR 前请先运行 `./align-ci.sh`，再运行 `./ci-check.sh`，确保本地与 CI 使用同一套规则且能通过流水线等价检查。
- 若修改运行期行为，请补充或更新相应测试；若影响对外 API 或用户可见行为，请同步更新本文档或相关 rustdoc。

向本仓库贡献内容即表示您同意以 [Apache License, Version 2.0](LICENSE)（与本项目相同）授权您的贡献。

## 许可证与版权

Copyright (c) 2026. Haixing Hu.

本软件依据 [Apache License, Version 2.0](LICENSE) 授权；完整许可文本见仓库根目录的 `LICENSE` 文件。

## 作者与维护

**Haixing Hu** — Qubit Co. Ltd.

| | |
| --- | --- |
| **源码仓库** | [github.com/qubit-ltd/rs-executor](https://github.com/qubit-ltd/rs-executor) |
| **API 文档** | [docs.rs/qubit-executor](https://docs.rs/qubit-executor) |
| **Crate 发布** | [crates.io/crates/qubit-executor](https://crates.io/crates/qubit-executor) |
