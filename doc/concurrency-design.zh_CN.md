# 并发设计说明

本文记录维护任务 handle、服务接纳和调度逻辑时应保留的并发契约。[English version](concurrency-design.md) 描述相同的保证与限制。

## 职责边界

[`Executor`](../src/executor/executor.rs) 描述一种执行策略。 [`ExecutorService`](../src/service/executor_service.rs) 增加托管接纳、生命周期管理和已接纳任务跟踪。 [`task::spi`](../src/task/mod.rs) 中的任务 SPI 让提供者把自己的调度机制连接到标准 handle 和完成端点。提供者负责队列、worker、同步和排序；SPI 不提供调度器，也不规定统一的队列策略。[`RecoverableExecutorService`](../src/service/recoverable_executor_service.rs) 是一种可选扩展，供能够在拒绝时交还原 callable 的提供者实现，并非所有服务都必须支持。

## 任务状态与结果发布

主要生命周期是从 `Pending` 到 `Running`，再到终态；也可以从 `Pending` 到 `Cancelled` 或 `Dropped`。竞争的终态转换通过原子状态更新进行，因此只有一个转换能够获胜。安装终态与发送结果是两个步骤：[`TaskState::publish_terminal_result`](../src/task/task_state.rs) 在状态转换之后发送结果。因此，即使 `is_done()` 已返回 true，`try_get()` 仍可能处于 pending。回归测试 [`test_terminal_state_can_precede_result_publication`](../src/task/task_state.rs) 通过持有 sender 锁验证这个窗口；[`test_terminal_transition_has_one_winner`](../src/task/task_state.rs) 验证终态竞争只有一个赢家。

## 接纳 hook 与 worker 排序

提供者应在提交成功后才接纳任务。启用 hook 时，[`TaskSlot::accept`](../src/task/task_slot.rs) 会同步调用 accepted 回调。如果要求 accepted 事件有序，提供者必须在发布可取消 handle 或允许 worker 启动前完成这次调用。内置 thread-per-task 路径使用 [`TaskAdmissionGate`](../src/task/task_admission_gate.rs)，让已创建的 worker 等待接纳发布。对于在 `accept` 返回前取消或执行等不支持的调用顺序，SPI 不会自动补足同步。未启用 hook 时，open gate 是空操作；自定义队列仍须定义自己的接纳和 worker 排序，不应忽略所需同步而照搬启用 hook 时的方案。相关覆盖包括 [`test_thread_per_task_executor_service_hook_events_are_ordered`](../tests/service/thread_per_task_executor_service_tests.rs) 以及该文件中的 service admission race 测试。

## 队列所有权、stop 与终止

任务状态说明单个任务发生了什么。队列成员关系表示仍由调度器持有的条目。`worker_active` 表示条目已从队列分离并由 worker 持有；`stop_draining` 表示 `stop` 正在调度器监视器之外取消队列条目。[`SchedulerState::can_terminate`](../src/schedule/scheduler_state.rs) 要求生命周期不再是 running、队列为空、没有活动 worker 条目且没有 stop drain。[`SchedulerCore::stop`](../src/schedule/scheduler_core.rs) 请求停止后可以在 worker 已领取任务时返回；它无法强制中断任意正在运行的 Rust 代码。并发 drain 和取消 panic 情况由 [`test_concurrent_stop_waits_for_first_drain`](../src/schedule/scheduler_core.rs) 与 [`test_stop_discards_later_cancel_panics_and_drains_all_entries`](../src/schedule/scheduler_core.rs) 覆盖。

## 回调、析构与 panic

hook 在发布事件的线程上同步执行，可能阻塞该线程。通过本 crate 的标准 `notify_*` 辅助函数调用时，hook 的普通 unwind 由 [`contain_hook_panic`](../src/hook/task_hook.rs) 捕获；告警日志器的 panic 也会被隔离。直接调用 `TaskHook::on_*` 方法不会自动获得这一保护。panic 隔离不能防止阻塞或死锁，不保证资源立即释放，也无法恢复 abort 型 panic 或 unwind 期间的双重 panic。现有回归覆盖了 [hook panic 隔离](../tests/hook/hook_tests.rs)和 [hook/日志器 panic 恢复](../tests/hook_recovery_tests.rs)。

[`TaskState::publish_terminal_result`](../src/task/task_state.rs) 会在发送或丢弃结果时捕获普通 unwind；runner 路径也会在取消后丢弃尚未执行的 callable 时捕获 unwind（[`TaskSlot::start_and_complete`](../src/task/task_slot.rs)）。这不包含 abort 型 panic，也不包含 unwind 期间发生的 double panic。其他提供者回调或析构函数可能在这些捕获范围之外同步执行。提供者可能在持有调度锁时调用回调；不要在回调中调用需要获取同一锁的操作。生命周期或队列大小查询也可能获取该锁，具体取决于提供者实现，请查阅提供者的回调契约。内置的 `SingleThreadScheduledExecutorService` 当前没有公开的任务钩子配置入口，但内部定时条目仍可能使用钩子；其接纳回调在调度器监视器持有期间运行，不得重入需要该监视器的操作。独立适配器契约套件在[恢复测试](https://github.com/qubit-ltd/rs-executor/blob/main/tests/adapter_contracts/tests/recoverable_contract_tests.rs)中覆盖重试和关闭导致的拒绝。不要在 callback 内重入需要其当前持有锁的操作，也不要在 hook 内等待同一任务或服务终止。

## 取消、清理与 join

取消是任务状态决策，不表示所有捕获值都已析构。取消结果可能先于并发取消路径完成队列 callable 的析构而被观察到。服务终止表示提供者的已接纳工作条件已满足；它不承诺所有 OS 线程都已 join、线程局部析构已运行，或外部并发取消调用已经返回。这些边界记录在 [`ExecutorService`](../src/service/executor_service.rs)；定时任务取消和调度 drain 分别由 [`ScheduledTaskHandle::cancel`](../src/schedule/scheduled_task_handle.rs) 与 [`SchedulerCore::stop`](../src/schedule/scheduler_core.rs) 实现。服务终止和取消行为由 [`thread_per_task_executor_service_tests.rs`](../tests/service/thread_per_task_executor_service_tests.rs) 与 [`single_thread_scheduled_executor_service_tests.rs`](../tests/schedule/single_thread_scheduled_executor_service_tests.rs) 覆盖。

## 可恢复提交

可恢复拒绝会把原 callable 的所有权交还调用方；该 callable 未被接纳，也不应已经执行。共享契约见 [`RejectedTask`](../src/service/rejected_task.rs) 和 [`RecoverableExecutorService`](../src/service/recoverable_executor_service.rs)。提供者可以保留返回本地拒绝类型的 inherent 方法，并通过共享 trait 转发拒绝原因和 callable。这两个 API 表面兼容，但拒绝类型不同。独立适配器契约套件在[恢复测试](https://github.com/qubit-ltd/rs-executor/blob/main/tests/adapter_contracts/tests/recoverable_contract_tests.rs)中覆盖重试和因关闭导致的拒绝。

## 证据与限制

以上链接将每项不变量与实现及回归覆盖对应起来。根目录的 `cargo test` 不会运行独立的 `tests/adapter_contracts` Cargo workspace；验证适配器行为时必须显式运行该 workspace。单元测试和集成测试覆盖若干选定的交错时序，并未覆盖所有可能的线程调度，也不构成对全部并发执行的形式化证明。
