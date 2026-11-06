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

已接受任务需要排空时调用 `shutdown()`；需要尽力取消排队或尚未开始的任务时调用 `stop()`。返回的 `StopReport` 提供实现观察到的 `queued`、`running` 和 `cancelled` 快照；并发状态转换期间，这些计数不一定与每个 handle 的状态严格一致。`stop()` 不能强制中断已经运行的任意 Rust 代码、阻塞调用或 OS 线程任务。对于单线程定时服务，stop 只排空仍在队列中的条目；worker 已领取的任务之后仍可能启动，活动计数也可能短暂包含状态仍为 `Pending` 的 handle。`wait_termination()` 等待服务定义的终止条件，不代表 join 所有 OS 线程或析构线程局部存储。

## 泛型异步等待

`ExecutorService` 保证结果 handle 支持观察结果，但泛型关联类型并未承诺实现 `IntoFuture`。需要等待结果的泛型函数应明确写出该约束。跟踪 handle 对应地约束 `TrackedHandle`：

```rust
use std::future::IntoFuture;
use qubit_executor::{ExecutorService, TaskResult};

async fn calculate<S>(service: &S) -> Result<usize, Box<dyn std::error::Error>>
where
    S: ExecutorService,
    S::ResultHandle<usize, std::io::Error>:
        IntoFuture<Output = TaskResult<usize, std::io::Error>>,
{
    let handle = service.submit_callable(|| Ok::<usize, std::io::Error>(42))?;
    Ok(handle.await?)
}

async fn tracked<S>(service: &S) -> TaskResult<(), std::io::Error>
where
    S: ExecutorService,
    S::TrackedHandle<(), std::io::Error>:
        IntoFuture<Output = TaskResult<(), std::io::Error>>,
{
    service.submit_tracked(|| Ok::<(), std::io::Error>(())).expect("accepted").await
}

fn main() {}
```

`.await` 会在异步任务等待时挂起该任务。`get()` 是同步方法，会阻塞当前线程；应在同步代码中使用，不要在异步任务里用它替代 await。`IntoFuture` 不保证返回的 Future 实现 `Send`；只有调用场景（例如多线程任务 spawn）需要时，才额外约束该 Future 为 `Send`。

## 异步超时后取消

具体的定时任务 handle 可以在原 handle 被 `into_future()` 消费前提供 `TaskCancellationHandle`。这样轮询结果 future 期间仍可取消任务。示例使用标准库实现的小型轮询循环和截止时间辅助函数，因此无需选择异步运行时：

```rust
use std::error::Error;
use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

use qubit_executor::{
    CancelResult, ExecutorService, ScheduledExecutorService,
    SingleThreadScheduledExecutorService, TaskExecutionError,
};

fn timeout<'a, F: Future>(
    duration: Duration,
    mut future: Pin<&'a mut F>,
) -> impl Future<Output = Option<F::Output>> + 'a {
    let deadline = Instant::now() + duration;
    let mut timer_started = false;
    std::future::poll_fn(move |context| {
        if Instant::now() >= deadline {
            return Poll::Ready(None);
        }
        if let Poll::Ready(output) = future.as_mut().poll(context) {
            return Poll::Ready(Some(output));
        }
        if !timer_started {
            timer_started = true;
            let waker = context.waker().clone();
            let remaining = deadline.saturating_duration_since(Instant::now());
            thread::spawn(move || {
                thread::sleep(remaining);
                waker.wake();
            });
        }
        Poll::Pending
    })
}

struct ThreadWaker(thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let service = SingleThreadScheduledExecutorService::new("cancel-after-timeout")?;
    let task = service.schedule_callable(Duration::from_secs(3600), || Ok::<usize, ()>(42))?;
    let cancellation = task.cancellation_handle();
    let mut result = Box::pin(task.into_future());

    if timeout(Duration::from_millis(10), result.as_mut())
        .await
        .is_none()
    {
        assert_eq!(cancellation.cancel(), CancelResult::Cancelled);
    }
    let outcome = timeout(Duration::from_secs(5), result.as_mut())
        .await
        .expect("cancelled task should finish within five seconds");
    assert!(matches!(outcome, Err(TaskExecutionError::Cancelled)));

    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    block_on(run())
}
```

必须在消费 handle 前获取端点。丢弃结果 future 或取消端点都不会取消任务。只有任务仍处于 Pending 时取消才能获胜；它不能停止已经运行的任意代码。适用时应处理 `AlreadyRunning`、`AlreadyFinished` 和 `Unsupported`。调用 `cancel()` 可能同步清理执行器状态或析构捕获值，因此可能阻塞或 panic。保留端点会延长共享任务状态及取消回调所捕获状态的寿命；不再需要时应及时释放。

`TaskCancellationHandle::new` 是供执行器提供者使用的适配入口。回调必须保留提供者完整的取消路径和记账。例如，Rayon 适配器必须保留 job 清理和 `finish_cancelling`；Tokio blocking 适配器必须保留排队任务记账及 `AbortHandle::abort`。旧的通用执行器 trait 不保证存在取消 handle 方法，因此泛型代码需要使用具体 handle 类型或定义自己的扩展约束。这些外部集成说明仅供迁移参考；Rayon 和 Tokio 下游 handle 类型尚未完成适配。

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

如需限制队列，可通过 `SingleThreadScheduledExecutorService::builder("bounded-scheduler").queue_capacity(128).build()?` 构建服务。容量只统计仍在队列中的条目；worker 已领取的一个任务不计入，因此队列最多等待 128 个任务，另加该活动任务。默认仍为无界队列。取消并移除条目或 worker 领取条目后会释放队列位置。队列满时提交返回 `SubmissionError::Saturated`；shutdown 或 stop 后提交返回 `Shutdown`。

该实现会在唯一的调度线程上运行到期任务，因此定时任务应保持短小；较重的工作应视情况转交给其他服务。

### 跟踪、取消与任务钩子

需要状态和任务开始前取消能力时，使用 `submit_tracked_callable`。`cancel()` 不保证已经开始运行的任务会停止；应同时检查返回的 `CancelResult` 和最终 `TaskResult`。对于 `DelayExecutor` 和 `ScheduleExecutor`，取消会唤醒正在等待延迟的工作线程，使它能够在截止时刻前释放捕获的任务。它们仍是每个任务创建一个 OS 线程；得到 `Cancelled` 不表示工作线程已经退出。丢弃 tracked handle 也不会取消任务；按值 await 会等待任务结果。取消调用移除定时任务时，捕获值会在调用线程同步析构，析构可能阻塞。若另一个线程仍在执行这段清理，服务仍可能报告终止；应用必须自行等待并发取消调用结束。

实现 `TaskHook` 可以观察 `on_accepted`、`on_rejected`、`on_started` 和 `on_finished`。被拒绝的提交只触发 `on_rejected`，不会获得 task id。已接受任务先触发 `on_accepted`；任务在开始前取消时，可能触发没有 `on_started` 的 `on_finished`。钩子在发布事件的线程上同步运行，不会转发到专用观察线程。根据执行器和 SPI 调用方，钩子运行时可能持有内部锁或调用方自己的锁。不要在钩子中重入同一个 `TaskSlotCell`，也不要等待当前任务或所属服务终止。钩子应短小且非阻塞；记录原子计数或使用 `try_send` 等非阻塞通知都是合适做法。执行器会隔离钩子 panic。

## 错误与诊断

提交错误会在任务接受前返回：

- `SubmissionError::Shutdown`：shutdown 或 stop 已关闭任务接收。
- `SubmissionError::Saturated`：具体服务暂时无法再接受更多任务。
- `SubmissionError::InvalidDeadline`：定时延迟超出目标平台 `Instant` 可以表示的范围。
- `SubmissionError::WorkerSpawnFailed`：创建工作线程失败，并携带底层错误。

任务接受后，应从 handle 读取结果，而不是继续看提交结果。`TaskExecutionError::Failed(E)` 是任务自己的错误；`Panicked`、`Cancelled` 和 `Dropped` 分别表示执行或完成阶段的对应状态。`Dropped` 表示已接受任务的 completion endpoint 在没有显式结果的情况下被放弃；`Cancelled` 表示任务开始前取消。不同清理路径的 unwind 行为如下：

- TaskRunner 发现任务在开始前已取消并丢弃 callable 时，普通清理 unwind 会被隔离，结果仍为 Cancelled；这不覆盖 aborting panic 或展开期间的双重 panic。
- 对于已经启动的任务，TaskRunner 会将 callable 的普通 unwind（包括捕获值析构）转换为 Panicked；这不覆盖 aborting panic 或展开期间的双重 panic。
- [`ScheduledTaskHandle::cancel`](https://docs.rs/qubit-executor/latest/qubit_executor/schedule/struct.ScheduledTaskHandle.html#method.cancel) 会先发布取消状态，再同步销毁被移除的排队任务。捕获值析构 panic 可以传播给调用方；已发布的取消状态不会回滚。
- [`ExecutorService::stop`](https://docs.rs/qubit-executor/latest/qubit_executor/service/trait.ExecutorService.html#tymethod.stop) 会 drain 定时任务队列，尽力处理其余条目并恢复 drain 记账，然后恢复首个清理 panic。取消结果与 panic 是否传播是两个独立结果。

如果 worker 已经领取定时任务，取消清理由 worker 的 unwind 边界处理；不能据此推断调用方执行取消清理时也会隔离 panic。

可使用 `lifecycle()`、`is_running()`、`is_shutting_down()`、`is_stopping()` 和 `is_terminated()` 检查服务状态。如果 `SingleThreadScheduledExecutorService` 的后台计时等待失败，服务会停止、取消尚未开始的任务，并以 `Shutdown` 拒绝新提交。

## 排障

- **提交被拒绝：** 检查 `lifecycle()` 和返回的 `SubmissionError`；调用 `shutdown()` 或 `stop()` 后不要继续提交。
- **结果尚未就绪：** `get()` 会阻塞；需要非阻塞读取时使用 `try_get()`。`is_done()` 只观察任务是否进入终态，不保证结果已经发布：即使它返回 `true`，`try_get()` 仍可能返回 `Pending`，`get()` 仍可能等待结果发布。保留 `Pending` 中的 handle，以便稍后轮询或 await；不要在必须负责运行任务的同一服务线程上等待结果。
- **终止等待不返回：** 检查仍在运行的已接受任务，尤其是无法中断的阻塞调用或 OS 线程任务。定时服务在 `shutdown()` 后仍保留已接受任务的原截止时刻；远期排队任务即使在没有运行任务时也可能让终止等待继续，包括丢弃服务 handle 触发优雅关闭之后。需要保留这些任务时继续等待；需要取消个别尚未开始的任务时取消对应的 tracked handle；需要取消仍排队的条目时调用 `stop()`。`wait_termination_timeout()` 超时只表示服务尚未报告终止，不会自动停止服务。`stop()` 仍不能中断运行中的任务，worker 已领取的定时任务也可能在 `stop()` 返回后启动。
- **定时任务被拒绝：** 减小延迟，或改用目标平台可以表示的 `Instant`。
- **工作线程无法创建：** 检查 `WorkerSpawnFailed` 中的底层错误，以及 builder 配置的线程名或栈大小。

## 限制与最佳实践

`ThreadPerTaskExecutor` 和 `ThreadPerTaskExecutorService` 会为每个已接受任务创建一个 OS 线程，因此应用应自行设置并发上限。`SingleThreadScheduledExecutorService` 只有一个调度线程，不能替代有界工作线程池。`stop()` 和丢弃 service handle 都不保证立即中断任务或释放资源。需要清理时，先停止新提交、请求 `shutdown()` 或 `stop()`，等待应用发起的取消调用结束，再等待 `wait_termination()`，并按外部资源所有者的协议完成清理。服务等待不保证 join 所有 OS 线程、析构线程局部存储，也不会释放仍由 handle、`Arc` 或外部线程持有的资源。

## 延伸阅读

- [README](../README.zh_CN.md)
- [English user guide](user_guide.md)
- [API 文档](https://docs.rs/qubit-executor)
- [Crates.io](https://crates.io/crates/qubit-executor)
