# qubit-executor User Guide

[简体中文](user_guide.zh_CN.md) · [README](../README.md) · [API documentation](https://docs.rs/qubit-executor)

## Purpose and Audience

This guide targets Rust library and application authors using `qubit-executor` 0.9. It explains how to choose between the execution-strategy abstraction `Executor` and a lifecycle-managed `ExecutorService`, observe task results, schedule work, and shut services down without assuming that dropping a handle waits for worker resources.

The crate is an execution abstraction. It does not select an async runtime, provide a bounded thread pool, or forcibly interrupt arbitrary Rust code.

## Conceptual Model

`Executor` describes how one accepted task runs. Its `call` and `execute` methods return a `TrackedTask`, and the outer `Result` reports submission failure only.

The concrete strategy defines invocation count and may retry a task. Generic callers must not assume at-most-once invocation from the `Executor` bound alone; check the concrete strategy before submitting work with side effects.

`ExecutorService` manages admission and lifecycle. An accepted `submit_callable` returns a task-result handle; `Ok(handle)` means acceptance, not successful completion. A service moves through these states:

| State | Meaning |
| --- | --- |
| `Running` | New submissions may be accepted. |
| `ShuttingDown` | `shutdown()` rejects new work and lets accepted work finish. |
| `Stopping` | `stop()` rejects new work and requests best-effort cancellation or abort. |
| `Terminated` | No accepted work remains active after shutdown or stop. |

`TaskHandle` exposes `get`, `try_get`, `is_done`, and by-value async waiting. `TrackedTask` additionally exposes `status`, `task_id`, and pre-start `cancel`. A final result is represented by `TaskResult` and may be success, `Failed(E)`, `Panicked`, `Cancelled`, or `Dropped`.

## Scenario: Accept Work Without Choosing an Application Runtime

A library wants to run a fallible calculation and return its result, while an application decides whether execution is inline or on a separate thread. The success criterion is that the library can use the same `Executor` API and the caller can observe `42` through the returned handle.

For deterministic behavior, use `DirectExecutor`:

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

When the application needs the task to run on a separate OS thread, replace the implementation without changing the task shape:

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

The observable result is `42`. If submission itself fails, the outer `Result` contains `SubmissionError`; if the task is accepted but fails, panics, or is cancelled, the error is returned by the task handle.

## Installation and Minimal Configuration

Add the crate to `Cargo.toml`:

```toml
qubit-executor = "0.9"
```

The package declares Rust 1.94 as its minimum Rust version. Import the `Executor` trait before calling `call` or `execute`, and import `ExecutorService` before using service methods supplied by the trait.

## Core Workflow: Managed Work

Use `ThreadPerTaskExecutorService` when a basic managed service is sufficient:

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

Call `shutdown()` when accepted work should drain. Call `stop()` when queued or unstarted work should be cancelled where possible. The returned `StopReport` exposes implementation-observed `queued`, `running`, and `cancelled` counts; they are snapshots and need not match every handle's status during a transition. Stopping is best effort and cannot forcibly interrupt already-running arbitrary Rust code, blocking calls, or OS-thread work. For the single-thread scheduled service, stop drains entries still queued; a worker-claimed task may still start afterward, and the active count can include a task whose handle remains `Pending` briefly. `wait_termination()` waits for the service's termination condition, not a join of every OS thread or thread-local destructor.

## Generic asynchronous waiting

`ExecutorService` guarantees that its result handle supports result observation, but its generic associated type does not promise `IntoFuture`. A generic function that awaits the result therefore states that bound explicitly. Tracked handles use the corresponding bound on `TrackedHandle`. Add `CancellableTaskHandle` when generic code must preserve the independent cancellation endpoint before consuming the handle:

```rust
use std::future::IntoFuture;
use qubit_executor::{CancellableTaskHandle, ExecutorService, TaskResult};

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

async fn tracked_with_cancellation<S>(service: &S) -> TaskResult<(), std::io::Error>
where
    S: ExecutorService,
    S::TrackedHandle<(), std::io::Error>: CancellableTaskHandle<(), std::io::Error>
        + IntoFuture<Output = TaskResult<(), std::io::Error>>,
{
    let handle = service.submit_tracked(|| Ok::<(), std::io::Error>(())).expect("accepted");
    let cancellation = CancellableTaskHandle::cancellation_handle(&handle);
    let result = handle.into_future().await;
    drop(cancellation);
    result
}

fn main() {}
```

`.await` suspends an async task while it waits. `get()` is synchronous and blocks the current thread, so use it in synchronous code rather than inside async work. `IntoFuture` does not guarantee a `Send` future; add a `Send` bound for the future only when the calling context, such as a multithreaded task spawn, requires it.

## Cancel after an async timeout

A concrete scheduled handle can provide a `TaskCancellationHandle` before the original handle is consumed by `into_future()`. This keeps cancellation available while the result future is being polled. The example uses a small standard-library polling loop and a deadline helper so it can run without choosing an async runtime:

```rust,executor_run
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

The endpoint must be obtained before consuming the handle. Dropping the result future or cancellation endpoint does not cancel the task. Cancellation only wins while the task is pending; it cannot stop arbitrary code that is already running, and callers should handle `AlreadyRunning`, `AlreadyFinished`, and `Unsupported` where applicable. Calling `cancel()` may synchronously clean up executor state or captured values, so it can block or panic. Retaining an endpoint extends the lifetime of the shared task state and any state captured by its cancellation callback; drop it when it is no longer needed.

`TaskCancellationHandle::new` is an adapter entry point for executor providers. Its callback must preserve the provider's complete cancellation path and accounting. For example, a Rayon adapter must retain job cleanup and `finish_cancelling`; a Tokio blocking adapter must retain queued-task accounting and `AbortHandle::abort`. `CancellableTaskHandle` is an opt-in extension trait available from `qubit-executor 0.9.1`; existing `TrackedTaskHandle` implementations do not need to implement it unless callers require the endpoint. Adapter releases that implement this extension trait must depend on at least `qubit-executor 0.9.1`. The trait does not change the legacy generic executor contracts.

## Implement a custom executor

Implement `Executor` when an application already owns a queue or worker and wants to return the crate's standard tracked task handle. The application supplies the scheduling boundary; the SPI supplies completion and cancellation state.

For each submission, create a `TaskEndpointPair` and split it into a `TrackedTask` and `TaskSlot`. Put the slot and callable in a `Send` job, then send that job to the application-owned queue. A successful send is the admission decision. When a worker takes the job, call `slot.accept()` and run it with `TaskRunner::new(task).run(slot)`. If the queue is disconnected, return `SubmissionError::Shutdown`; dropping the rejected job also drops its slot, so it was never accepted.

The runnable version of this flow is [`examples/custom_executor.rs`](../examples/custom_executor.rs). It starts one application-owned worker, observes a result of `42`, closes and joins the worker, and checks the disconnected-queue rejection path. This is an introductory executor with one worker and an unbounded channel. It does not provide queue capacity limits or a managed service lifecycle; add those policies in the application if needed.

The example does not install hooks. An executor that uses hooks must finish `slot.accept()`, including the synchronous `on_accepted` callback, before returning the cancellable handle or allowing a worker to start the task. The SPI preserves lifecycle event ordering for supported use; it does not serialize cancellation or task start that races with or precedes acceptance. See [`TaskEndpointPair`](https://docs.rs/qubit-executor/latest/qubit_executor/task/spi/struct.TaskEndpointPair.html).

## Advanced Usage

### Delayed submission

`DelayExecutor` starts its configured relative delay only after its helper
thread passes the admission gate. Thread startup, scheduling, and synchronous
`on_accepted` hook time do not count toward the delay; with a hook, its
`on_accepted` callback must return before the gate opens. The task may start
later than the requested delay, with no real-time upper bound. In contrast,
`ScheduleExecutor` targets an absolute `Instant` deadline.

`SingleThreadScheduledExecutorService` owns one scheduler thread and accepts a `Duration` delay or an `Instant` deadline:

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

For an optional queue limit, build the service with `SingleThreadScheduledExecutorService::builder("bounded-scheduler").queue_capacity(128).build()?`. The capacity counts queued entries only; one worker-claimed task is outside the limit, so at most 128 tasks wait in the queue in addition to that task. The default remains unbounded. Cancelling and removing an entry or having the worker claim it frees a queue slot. A full queue rejects a submission with `SubmissionError::Saturated`; after shutdown or stop, submissions return `Shutdown`.

Submission methods consume callables by value. A `SubmissionError` reports why a task was rejected but does not return the callable, so keep the one-shot data in application-owned state before making a submission that may need retrying. In the example below, the queued filler makes the first rejection deterministic. Each attempt clones only an `Arc`; the payload is taken only if the callable actually runs, so rejecting and dropping an attempt leaves the payload available. After cancelling the filler to free the queue slot, the example resubmits and checks both the result and that the payload was consumed. Other producers may take the freed capacity first, so keep handling `Saturated` and retain the payload across further attempts. Return other submission errors to the caller rather than retrying them blindly.

```rust,executor_run
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use qubit_executor::CancelResult;
use qubit_executor::ExecutorService;
use qubit_executor::ScheduledExecutorService;
use qubit_executor::SingleThreadScheduledExecutorService;
use qubit_executor::SubmissionError;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = SingleThreadScheduledExecutorService::builder("retained-job")
        .queue_capacity(1)
        .build()?;
    let filler = service.schedule_callable(Duration::from_secs(3600), || Ok::<_, io::Error>(()))?;
    let retained = Arc::new(Mutex::new(Some(String::from("report"))));
    let submit = || {
        let retained = Arc::clone(&retained);
        service.schedule_callable(Duration::ZERO, move || {
            let payload = retained
                .lock()
                .expect("payload lock is not poisoned")
                .take()
                .ok_or_else(|| io::Error::other("payload already consumed"))?;
            Ok::<usize, io::Error>(payload.len())
        })
    };
    match submit() {
        Err(SubmissionError::Saturated) => {
            assert!(retained.lock().expect("payload lock").is_some());
        }
        Err(error) => return Err(error.into()),
        Ok(_) => panic!("the queue should still contain the far-future filler"),
    }
    assert_eq!(filler.cancel(), CancelResult::Cancelled);
    let handle = submit()?;
    assert_eq!(handle.get()?, 6);
    assert!(retained.lock().expect("payload lock").is_none());
    service.shutdown();
    service.wait_termination();
    Ok(())
}
```

The filler and fixed capacity are for this synchronous demonstration. In an application, trigger another attempt from a capacity event, use a finite retry budget, or place the work in an application queue; do not busy-loop. Once a task is accepted, use its handle to process execution errors and do not automatically resubmit failed work, which could repeat side effects. This example uses blocking `get()` and is for synchronous code, not an async task. The `qubit-execution-services` facade's capacity-wait API applies to Blocking, CPU, and TokioBlocking domains; it is not a wait API for this scheduled service. The scheduled service keeps its existing consuming submission API, so retry ownership stays with the caller; this avoids adding a new submission contract. Retaining data across attempts requires application-managed state and synchronization.

Keep scheduled task bodies short. This implementation runs due tasks on its single scheduler thread; hand heavier work to another service when appropriate.

### Tracking, cancellation, and hooks

Use `submit_tracked_callable` when status and pre-start cancellation are needed. `cancel()` is not a guarantee that a task already running will stop; inspect the returned `CancelResult` and final `TaskResult`. For `DelayExecutor` and `ScheduleExecutor`, cancellation wakes the worker's pending delay so it can release the captured task before its deadline. They still create one OS thread per task, and `Cancelled` does not mean that the worker thread has already exited. Dropping a tracked handle also does not cancel the task; awaiting it waits for its result. When a cancellation call removes a scheduled task, destroying its captured values can block on that caller's thread. A service may terminate while another thread is still in that cleanup, so applications must wait for their concurrent cancellation calls to finish separately.

Implement `TaskHook` to observe `on_accepted`, `on_rejected`, `on_started`, and `on_finished`. Rejected submissions receive only `on_rejected` and no task id. An accepted task receives `on_accepted` before later events; a task cancelled before starting may receive `on_finished` without `on_started`. Hooks run synchronously on the thread publishing each event, not on a dedicated observer thread. Depending on the executor and SPI caller, a hook may run while an internal or caller-owned lock is held. Do not re-enter the same `TaskSlotCell` or wait for the current task or its owning service to terminate from a hook. Keep hooks short and non-blocking; recording an atomic counter or using a non-blocking notification such as `try_send` is suitable. Hook panics are contained by the executor.

## Recover a rejected submission

Some providers can implement the optional `RecoverableExecutorService` extension. Its `try_submit_callable` returns `Err(RejectedTask<C>)` only when the callable was not accepted. The `RejectedTask` owns the original callable and its captured values; split it with `into_parts()` to inspect the `SubmissionError` and decide what to do with that callable.

```rust
use qubit_executor::{RecoverableExecutorService, RejectedTask};

fn submit_or_recover<S, C>(service: &S, task: C)
    -> Result<S::ResultHandle<usize, ()>, RejectedTask<C>>
where
    S: RecoverableExecutorService,
    C: FnMut() -> Result<usize, ()> + Send + 'static,
{
    service.try_submit_callable(task)
}

fn main() {}
```

```rust,executor_run
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use qubit_executor::{DirectExecutor, Executor, RejectedTask};
use qubit_executor::{SubmissionError, TrackedTask};

struct Probe(Arc<AtomicUsize>);

impl Drop for Probe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn submit<C>(ready: bool, task: C)
    -> Result<TrackedTask<usize, ()>, RejectedTask<C>>
where
    C: FnMut() -> Result<usize, ()> + Send + 'static,
{
    if !ready {
        return Err(RejectedTask::new(SubmissionError::Saturated, task));
    }
    Ok(DirectExecutor::new().call(task).expect("direct submission"))
}

fn main() {
    let calls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let task_calls = Arc::clone(&calls);
    let probe = Probe(Arc::clone(&drops));
    let payload = String::from("report");
    let task = move || {
        let _keep = &probe;
        task_calls.fetch_add(1, Ordering::SeqCst);
        Ok::<usize, ()>(payload.len())
    };

    let rejected = match submit(false, task) {
        Err(rejected) => rejected,
        Ok(_) => panic!("the simulated admission gate is closed"),
    };
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let (reason, task) = rejected.into_parts();
    assert!(matches!(reason, SubmissionError::Saturated));
    let handle = submit(true, task).expect("the simulated gate is open");
    assert_eq!(handle.get().expect("task result"), 6);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
```

This generic function shows the real provider call signature for the closure use case; it does not cover every `Callable` implementation. The following local `submit` function is only a deterministic ownership demonstration, not a `RecoverableExecutorService` implementation or a service-capacity test. It retries once. If a real provider rejects the second attempt, retain its `RejectedTask` or choose an explicit disposition. A `Saturated` rejection can justify a later retry, but does not promise capacity will become available. Use a capacity notification, a finite retry policy with a delay, or an application queue; do not busy-loop. `Shutdown` means admission is closed, so retrying the same service is not useful. In every branch, the application chooses how long to retain the rejected callable and when to drop or persist it.

Dropping an async result future abandons that wait; it does not cancel work already accepted. A rejected callable is different: it was not accepted, and ownership remains with `RejectedTask` until the caller extracts or drops it. An `Ok(handle)` still means acceptance only, not that execution has started or succeeded.

This optional trait is a provider boundary. Rayon and Tokio integrations need their provider to implement it, or an application can define a local newtype adapter around a foreign service and implement the trait for that local type. Rust's orphan rules prevent an external crate from implementing this crate's trait directly for both an external provider type and this external trait. The built-in `ThreadPerTaskExecutorService` and scheduled services do not currently implement `RecoverableExecutorService`.

The `qubit-execution-services` blocking domain's `OwnedWaitTask` has not migrated to this extension. This change makes no allocation-reduction claim.

## Errors and Diagnostics

Submission errors are returned before a task is accepted:

- `SubmissionError::Shutdown` means shutdown or stop has closed admission.
- `SubmissionError::Saturated` indicates that a concrete service cannot accept more work.
- `SubmissionError::InvalidDeadline` means a scheduled delay cannot be represented by the target platform's `Instant` range.
- `SubmissionError::WorkerSpawnFailed` contains the worker creation error.

After acceptance, inspect the handle rather than the submission result. `TaskExecutionError::Failed(E)` is the task's own error; `Panicked`, `Cancelled`, and `Dropped` describe execution or completion conditions. `Dropped` means an accepted completion endpoint was abandoned without an explicit result; `Cancelled` means cancellation won before execution started. The unwind behavior depends on the cleanup path:

- When TaskRunner observes that cancellation won before start and discards the callable, ordinary cleanup unwinding is contained and the result remains Cancelled. This does not cover aborting panics or a double panic during unwinding.
- For a task that starts, TaskRunner converts ordinary callable unwinding, including captured-value destruction, into Panicked. This does not cover aborting panics or a double panic during unwinding.
- [`ScheduledTaskHandle::cancel`](https://docs.rs/qubit-executor/latest/qubit_executor/schedule/struct.ScheduledTaskHandle.html#method.cancel) publishes cancellation before synchronously destroying a removed queued task. A panic from captured-value destruction can propagate to the caller; the published cancellation is not rolled back.
- [`ExecutorService::stop`](https://docs.rs/qubit-executor/latest/qubit_executor/service/trait.ExecutorService.html#tymethod.stop) drains queued scheduled tasks, attempts to process the remaining entries and restore drain bookkeeping, then resumes the first cleanup panic. The cancellation result and whether a panic propagates are separate outcomes.

When a worker has already claimed a scheduled task, cancellation cleanup follows the worker's unwind boundary; that does not imply that caller-side cancellation cleanup is also contained.

Use `lifecycle()`, `is_running()`, `is_shutting_down()`, `is_stopping()`, and `is_terminated()` to diagnose service state. If the background timer wait in `SingleThreadScheduledExecutorService` fails, that service stops, cancels work that has not started, and rejects new submissions with `Shutdown`.

## Troubleshooting

- **A submission is rejected:** inspect `lifecycle()` and the returned `SubmissionError`; do not submit after `shutdown()` or `stop()`.
- **A result is not ready:** `get()` blocks. Use `try_get()` for non-blocking retrieval. `is_done()` observes a terminal task state, not result availability: result publication may still be in progress, so `try_get()` can return `Pending` and `get()` can still block after `is_done()` returns `true`. Retain the handle returned in `Pending` to poll or await later, and avoid waiting on the same service thread that must run the task.
- **Termination does not return:** locate accepted tasks that are still running, especially blocking calls or OS-thread work that cannot be interrupted. A scheduled service also retains accepted tasks' original deadlines after `shutdown()`; a far-future queued task can therefore keep termination pending even when no task is currently running, including after the service handle is dropped. Wait when those tasks should be preserved, cancel individual tracked handles before they start, or call `stop()` to cancel entries still queued. A `wait_termination_timeout()` timeout only means termination has not yet been reported; it does not stop the service. `stop()` still cannot interrupt running tasks, and a worker-claimed scheduled task may start after `stop()` returns.
- **A scheduled task is rejected:** reduce the delay or use an `Instant` in the target platform's representable range.
- **A worker cannot start:** inspect the inner error in `WorkerSpawnFailed` and the configured thread name or stack size on the builder.

## Limitations and Best Practices

`ThreadPerTaskExecutor` and `ThreadPerTaskExecutorService` create an OS thread for each accepted task, so applications should impose their own concurrency limit. `SingleThreadScheduledExecutorService` has one scheduler thread and is not a replacement for a bounded worker pool. Neither `stop()` nor dropping a service handle guarantees immediate interruption or resource release. For cleanup, stop new submissions, request `shutdown()` or `stop()`, wait for cancellation calls the application started, then wait for `wait_termination()` and use each external resource's own cleanup protocol. The service wait does not join every OS thread, wait for thread-local destructors, or release resources still owned by handles, `Arc`s, or external threads.

## Further Reading

- [README](../README.md)
- [简体中文用户手册](user_guide.zh_CN.md)
- [API documentation](https://docs.rs/qubit-executor)
- [Crates.io](https://crates.io/crates/qubit-executor)
