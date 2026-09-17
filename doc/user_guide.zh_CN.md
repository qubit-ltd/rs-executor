# qubit-executor 用户手册

## 使用场景

当库需要接收任务、但不希望绑定具体运行时时，使用本 crate 定义通用执行器
接口。应用可按场景选择线程池、单线程定时服务，或 Tokio、Rayon 集成。

## 安装

```toml
qubit-executor = "0.8"
```

一次性调用使用 `Executor`，需要生命周期管理时使用 `ExecutorService`。返回的
句柄支持 `get`、`wait` 和取消操作。

## 生命周期

创建服务并提交任务后，使用 `shutdown` 等待已提交任务完成，或使用 `stop` 取消
队列中的任务。进程退出前调用 `wait_termination`（或带超时版本）。`TaskHook`
可以观察接收、拒绝和完成事件；钩子应保持快速，且不要依赖正在观察的任务。

## 定时与错误

`SingleThreadScheduledExecutorService` 接受 `Duration` 延迟。平台时钟无法表示的
延迟会返回 `SubmissionError::InvalidDeadline`。已关闭服务返回
`SubmissionError::Shutdown`，创建工作线程失败返回
`SubmissionError::WorkerSpawnFailed`。

## 运行限制

每个线程任务服务都会为接收的任务创建一个操作系统线程，应用应自行设置并发上限。
定时服务使用一个工作线程。终止等待基于单调时钟，超长时长不会引发 panic；如果
在超时内尚未终止，仍会返回 `false`。

## 故障排查

- 提交被拒绝时检查 `lifecycle()`，服务开始关闭后停止提交。
- 终止等待超时时检查是否存在未返回的运行中任务。
- 延迟被拒绝时，减小延迟到目标平台 `Instant` 可以表示的范围内。

API 详情参见 [docs.rs](https://docs.rs/qubit-executor)。
