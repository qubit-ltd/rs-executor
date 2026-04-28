# Qubit Executor

[![CircleCI](https://circleci.com/gh/qubit-ltd/rs-executor.svg?style=shield)](https://circleci.com/gh/qubit-ltd/rs-executor)
[![Coverage Status](https://coveralls.io/repos/github/qubit-ltd/rs-executor/badge.svg?branch=main)](https://coveralls.io/github/qubit-ltd/rs-executor?branch=main)
[![Crates.io](https://img.shields.io/crates/v/qubit-executor.svg?color=blue)](https://crates.io/crates/qubit-executor)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Documentation](https://img.shields.io/badge/docs-English-blue.svg)](README.md)

面向 Rust 的 executor 抽象与任务结果原语。

## 概览

Qubit Executor 提供 Qubit Rust 并发 crate 共享的最小执行 API。它把轻量级执行策略与托管 executor service 分开，并提供可复用的任务 handle，用于发布任务成功、任务失败、panic 或取消状态。

本 crate 刻意不依赖 Tokio、Rayon 或具体线程池。依赖运行时的实现放在更小的配套 crate 中，便于库作者只依赖自己需要的抽象层。

## 功能

- 提供策略级 `Executor` trait，用于执行单个任务并返回由实现决定的结果载体。
- 提供 `FutureExecutor` 标记 trait，表示执行结果具备 future 风格。
- 提供 `DirectExecutor`，用于确定性的同线程执行。
- 提供 `DelayExecutor`，用于延迟后再转交给另一个 executor。
- 提供 `ThreadPerTaskExecutor`，用于每个任务启动一个 OS 线程且不管理队列。
- 提供托管 `ExecutorService` trait，支持 `submit`、`submit_callable`、`shutdown`、`shutdown_now` 和等待终止。
- 提供 `ThreadPerTaskExecutorService` 作为基础托管服务实现。
- 提供 `TaskHandle`、`TaskCompletion`、`TaskExecutionError` 与 `TaskResult`，用于在多个 crate 之间共享任务完成语义。
- 通过 `RejectedExecution` 与 `ShutdownReport` 提供共享的拒绝执行原因和关闭报告类型。

## Executor 与 ExecutorService

`Executor` 是底层执行策略。它回答的是：“这个单个任务应该如何运行，以及用什么类型表示结果？”直接 executor 可以返回普通 `Result`，线程执行器可以返回 `TaskHandle`。

`ExecutorService` 是托管服务。它回答的是：“这个服务是否能接受任务、跟踪任务、关闭并最终终止？”`submit` 成功只表示服务接受了任务，不表示任务已经开始或成功完成。

## 任务结果

`TaskHandle` 表示已被接受的任务。它支持通过 `get` 阻塞等待、作为 `Future` 异步等待、通过 `is_done` 检查完成状态，以及在任务开始前进行尽力取消。

任务执行错误由 `TaskExecutionError` 表示：

- `Failed(E)` 表示任务返回了自己的错误值。
- `Panicked` 表示任务运行期间发生 panic。
- `Cancelled` 表示任务在产生结果前被取消。

## 快速开始

### 直接执行

```rust
use std::io;

use qubit_executor::executor::{DirectExecutor, Executor};

let executor = DirectExecutor;
let value = executor.call(|| Ok::<usize, io::Error>(40 + 2))?;
assert_eq!(value, 42);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### 每个任务一个线程

```rust
use std::io;

use qubit_executor::executor::{Executor, ThreadPerTaskExecutor};

let executor = ThreadPerTaskExecutor;
let handle = executor.call(|| Ok::<usize, io::Error>(40 + 2));
assert_eq!(handle.get()?, 42);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### 托管服务

```rust
use std::io;

use qubit_executor::service::{ExecutorService, ThreadPerTaskExecutorService};

let service = ThreadPerTaskExecutorService::new();
let handle = service.submit_callable(|| Ok::<usize, io::Error>(40 + 2))?;
assert_eq!(handle.get()?, 42);
service.shutdown();
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

版权所有 © 2026 Haixing Hu，Qubit Co. Ltd.。

本软件依据 [Apache License, Version 2.0](LICENSE) 授权；完整许可文本见仓库根目录的 `LICENSE` 文件。

## 作者与维护

**Haixing Hu** — Qubit Co. Ltd.

| | |
| --- | --- |
| **源码仓库** | [github.com/qubit-ltd/rs-executor](https://github.com/qubit-ltd/rs-executor) |
| **API 文档** | [docs.rs/qubit-executor](https://docs.rs/qubit-executor) |
| **Crate 发布** | [crates.io/crates/qubit-executor](https://crates.io/crates/qubit-executor) |
