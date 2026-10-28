# qubit-executor 用户手册

[English](user_guide.md) · [README](../README.zh_CN.md) · [API 文档](https://docs.rs/qubit-executor)

## 手册目标与读者

本手册面向使用 `qubit-executor` 0.9 的 Rust 库作者和应用开发者，说明如何在一次性 `Executor` 与需要生命周期管理的 `ExecutorService` 之间做选择，如何读取任务结果、安排定时任务，以及如何在不假设丢弃 handle 会等待工作线程的前提下完成服务收尾。

这个 crate 提供的是执行抽象，不负责选择异步运行时、不提供有界线程池，也不能强制中断任意 Rust 代码。

## 概念模型

`Executor` 描述一个已接受任务的运行方式。`call` 和 `execute` 返回 `TrackedTask`，外层 `Result` 只表示提交是否成功。

`ExecutorService` 负责接收任务和管理生命周期。`submit_callable` 成功后返回任务结果 handle；`Ok(handle)` 表示任务已被接受，并不表示任务已经成功完成。服务状态如下：

| 状态 | 含义 |
| --- | --- |
| `Running` | 可以继续接受新任务。 |
| `ShuttingDown` | `shutdown()` 拒绝新任务，并允许已接受任务完成。 |
| `Stopping` | `stop()` 拒绝新任务，并尽力取消或 abort 已接受任务。 |
| `Terminated` | shutdown 或 stop 之后，已接受任务均不再处于活动状态。 |

`TaskHandle` 提供 `get`、`try_get`、`is_done` 和按值异步等待。`TrackedTask` 还提供 `status`、`task_id` 和任务开始前的 `cancel`。最终结果由 `TaskResult` 表示，可能是成功、`Failed(E)`、`Panicked`、`Cancelled` 或 `Dropped`。

## 贯穿场景：不绑定应用运行时地接收任务

一个库希望执行可失败的计算并返回结果，但由应用决定任务是在当前线程执行还是在独立线程执行。成功标准是：库使用同一套 `Executor` API，调用方可以通过返回的 handle 观察到 `42`。

需要确定性行为时，使用 `DirectExecutor`：

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

如果应用需要在独立 OS 线程中执行，只需替换实现，任务形状无需变化：

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
use std::io;

use qubit_executor::{Executor, ThreadPerTaskExecutor};

let executor = ThreadPerTaskExecutor::new();
let handle = executor.call(|| Ok::<usize, io::Error>(40 + 2))?;
assert_eq!(handle.get()?, 42);
Ok(())
}
```

可观察结果是 `42`。若提交阶段失败，外层 `Result` 会包含 `SubmissionError`；若任务已经接受但随后失败、panic 或被取消，则应从任务 handle 读取对应错误。

## 安装与最小配置

在 `Cargo.toml` 中加入：

```toml
qubit-executor = "0.9"
```

该 package 声明的最低 Rust 版本是 1.94。调用 `call` 或 `execute` 前需要导入 `Executor` trait；使用服务 trait 提供的方法前需要导入 `ExecutorService`。

## 核心工作流：托管任务

基础的托管执行可以使用 `ThreadPerTaskExecutorService`：

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
use std::io;

use qubit_executor::{ExecutorService, ThreadPerTaskExecutorService};

let service = ThreadPerTaskExecutorService::new();
let handle = service.submit_callable(|| Ok::<usize, io::Error>(40 + 2))?;
assert_eq!(handle.get()?, 42);

service.shutdown();
assert!(service.wait_termination_timeout(std::time::Duration::from_secs(1)));
Ok(())
}
```

已接受任务需要排空时调用 `shutdown()`；需要尽力取消排队或尚未开始的任务时调用 `stop()`。返回的 `StopReport` 提供 `queued`、`running` 和 `cancelled` 数量。`stop()` 不能强制中断已经运行的任意 Rust 代码、阻塞调用或 OS 线程任务。若调用方必须等到服务没有活动任务，再使用 `wait_termination()`。

## 实现自定义执行器

如果应用已经拥有队列或工作线程，并希望返回 crate 标准的可跟踪任务句柄，可以实现 `Executor`。应用负责提供调度边界，SPI 负责提供任务完成与取消状态。

每次提交时，创建一个 `TaskEndpointPair`，将它拆分为 `TrackedTask` 和 `TaskSlot`。把 slot 与 callable 放入 `Send` 任务，再发送到应用自己的队列。发送成功即表示接纳；工作线程取出任务后调用 `slot.accept()`，并通过 `TaskRunner::new(task).run(slot)` 执行。若队列已断开，则返回 `SubmissionError::Shutdown`；被丢弃的拒绝任务也会丢弃其 slot，因此它从未被接受。

[`examples/custom_executor.rs`](../examples/custom_executor.rs) 提供了可运行版本：启动一个由应用拥有的工作线程，通过 handle 读取结果 `42`，关闭队列并等待线程退出，然后验证队列断开时的拒绝路径。该示例是只有一个工作线程和无界 channel 的入门执行器，不提供队列容量限制或托管服务生命周期；应用如有需要应自行增加这些策略。

示例没有安装 hook，因此会在返回 handle 后由工作线程接纳 slot。启用 hook 的执行器必须先完成 `slot.accept()`（包括同步执行完 `on_accepted` 回调），再向调用方返回可取消的 handle 或允许工作线程启动任务。SPI 会维护受支持调用方式下的生命周期事件顺序，但不会串行化与接纳并发或早于接纳完成的取消/启动。详见 [`TaskEndpointPair`](https://docs.rs/qubit-executor/latest/qubit_executor/task/spi/struct.TaskEndpointPair.html)。

## 进阶用法

### 延迟提交

`SingleThreadScheduledExecutorService` 拥有一个调度线程，支持传入 `Duration` 延迟或 `Instant` 截止时刻：

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
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
Ok(())
}
```

该实现会在唯一的调度线程上运行到期任务，因此定时任务应保持短小；较重的工作应视情况转交给其他服务。

### 跟踪、取消与任务钩子

需要状态和任务开始前取消能力时，使用 `submit_tracked_callable`。`cancel()` 不保证已经开始运行的任务会停止；应同时检查返回的 `CancelResult` 和最终 `TaskResult`。对于 `DelayExecutor` 和 `ScheduleExecutor`，取消会唤醒正在等待延迟的工作线程，使它能够在截止时刻前释放捕获的任务。它们仍是每个任务创建一个 OS 线程；得到 `Cancelled` 不表示工作线程已经退出。丢弃 tracked handle 也不会取消任务；按值 await 会等待任务结果。

实现 `TaskHook` 可以观察 `on_accepted`、`on_rejected`、`on_started` 和 `on_finished`。被拒绝的提交只触发 `on_rejected`，不会获得 task id。已接受任务先触发 `on_accepted`；任务在开始前取消时，可能触发没有 `on_started` 的 `on_finished`。钩子在发布事件的线程上同步运行，不会转发到专用观察线程。根据执行器和 SPI 调用方，钩子运行时可能持有内部锁或调用方自己的锁。不要在钩子中重入同一个 `TaskSlotCell`，也不要等待当前任务或所属服务终止。钩子应短小且非阻塞；记录原子计数或使用 `try_send` 等非阻塞通知都是合适做法。执行器会隔离钩子 panic。

## 错误与诊断

提交错误会在任务接受前返回：

- `SubmissionError::Shutdown`：shutdown 或 stop 已关闭任务接收。
- `SubmissionError::Saturated`：具体服务暂时无法再接受更多任务。
- `SubmissionError::InvalidDeadline`：定时延迟超出目标平台 `Instant` 可以表示的范围。
- `SubmissionError::WorkerSpawnFailed`：创建工作线程失败，并携带底层错误。

任务接受后，应从 handle 读取结果，而不是继续看提交结果。`TaskExecutionError::Failed(E)` 是任务自己的错误；`Panicked`、`Cancelled` 和 `Dropped` 分别表示执行或完成阶段的对应状态。`Dropped` 表示已接受任务的 completion endpoint 在没有显式结果的情况下被放弃；`Cancelled` 表示任务开始前取消胜出。`TaskRunner` 会将 callable 的普通 unwind（包括释放捕获值时发生的 panic）转换为 `Panicked`，但无法恢复 aborting panic 或进程终止。

可使用 `lifecycle()`、`is_running()`、`is_shutting_down()`、`is_stopping()` 和 `is_terminated()` 检查服务状态。如果 `SingleThreadScheduledExecutorService` 的后台计时等待失败，服务会停止、取消尚未开始的任务，并以 `Shutdown` 拒绝新提交。

## 排障

- **提交被拒绝：** 检查 `lifecycle()` 和返回的 `SubmissionError`；调用 `shutdown()` 或 `stop()` 后不要继续提交。
- **结果尚未就绪：** `get()` 会阻塞；轮询时使用 `try_get()`，只检查完成状态时使用 `is_done()`。不要在必须负责运行任务的同一服务线程上等待结果。
- **终止等待不返回：** 检查仍在运行的已接受任务，尤其是无法中断的阻塞调用或 OS 线程任务。
- **定时任务被拒绝：** 减小延迟，或改用目标平台可以表示的 `Instant`。
- **工作线程无法创建：** 检查 `WorkerSpawnFailed` 中的底层错误，以及 builder 配置的线程名或栈大小。

## 限制与最佳实践

`ThreadPerTaskExecutor` 和 `ThreadPerTaskExecutorService` 会为每个已接受任务创建一个 OS 线程，因此应用应自行设置并发上限。`SingleThreadScheduledExecutorService` 只有一个调度线程，不能替代有界工作线程池。`stop()` 和丢弃 service handle 都不保证立即中断任务或释放资源。需要确定性清理时，应请求 `shutdown()` 或 `stop()`，等待 `wait_termination()` 返回后再丢弃 handle。

## 延伸阅读

- [README](../README.zh_CN.md)
- [English user guide](user_guide.md)
- [API 文档](https://docs.rs/qubit-executor)
- [Crates.io](https://crates.io/crates/qubit-executor)
