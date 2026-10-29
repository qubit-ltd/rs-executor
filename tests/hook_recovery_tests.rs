// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::io;
use std::panic;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;

use log::LevelFilter;
use log::Log;
use log::Metadata;
use log::Record;
use qubit_executor::DelayExecutor;
use qubit_executor::DirectExecutor;
use qubit_executor::Executor;
use qubit_executor::ExecutorService;
use qubit_executor::ScheduleExecutor;
use qubit_executor::TaskStatus;
use qubit_executor::ThreadPerTaskExecutor;
use qubit_executor::ThreadPerTaskExecutorService;
use qubit_executor::hook::TaskHook;
use qubit_executor::hook::TaskId;
use qubit_executor::service::SubmissionError;

struct PanickingLogger;

impl Log for PanickingLogger {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, _: &Record<'_>) {
        if PANIC_WITH_DROP_PAYLOAD.load(Ordering::SeqCst) {
            panic::panic_any(PanicOnDrop);
        }
        panic!("logger panic");
    }

    fn flush(&self) {}
}

static LOGGER: PanickingLogger = PanickingLogger;
static PANIC_WITH_DROP_PAYLOAD: AtomicBool = AtomicBool::new(false);

struct PanicOnDrop;

impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        panic!("panic payload drop panic");
    }
}

struct PanickingHook {
    events: Mutex<Vec<String>>,
    finished: Mutex<Option<mpsc::Sender<()>>>,
}

impl PanickingHook {
    fn with_finished_signal() -> (Self, mpsc::Receiver<()>) {
        let (sender, receiver) = mpsc::channel();
        (
            Self {
                events: Mutex::new(Vec::new()),
                finished: Mutex::new(Some(sender)),
            },
            receiver,
        )
    }

    fn record(&self, event: String) {
        self.events
            .lock()
            .expect("events lock should remain available")
            .push(event);
    }

    fn events(&self) -> Vec<String> {
        self.events.lock().expect("events lock should remain available").clone()
    }
}

impl TaskHook for PanickingHook {
    fn on_accepted(&self, task_id: TaskId) {
        self.record(format!("accepted:{}", task_id.get()));
        panic!("accepted hook panic");
    }

    fn on_started(&self, task_id: TaskId) {
        self.record(format!("started:{}", task_id.get()));
        panic!("started hook panic");
    }

    fn on_finished(&self, task_id: TaskId, status: TaskStatus) {
        self.record(format!("finished:{}:{status:?}", task_id.get()));
        if let Some(sender) = self
            .finished
            .lock()
            .expect("finished signal lock should remain available")
            .take()
        {
            let _ = sender.send(());
        }
        panic!("finished hook panic");
    }

    fn on_rejected(&self, error: &SubmissionError) {
        self.record(format!("rejected:{error:?}"));
        panic!("rejected hook panic");
    }
}

struct DropSignal(mpsc::Sender<()>);

impl Drop for DropSignal {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

fn assert_lifecycle(hook: &PanickingHook, context: &str) {
    let events = hook.events();
    assert_eq!(
        events.len(),
        3,
        "{context}: expected every lifecycle callback, got {events:?}"
    );
    assert!(
        events[0].starts_with("accepted:"),
        "{context}: accepted must be first: {events:?}"
    );
    let task_id = events[0].strip_prefix("accepted:").expect("accepted event");
    assert_eq!(events[1], format!("started:{task_id}"), "{context}: {events:?}");
    assert_eq!(
        events[2],
        format!("finished:{task_id}:Succeeded"),
        "{context}: {events:?}"
    );
}

fn wait_for_drop(receiver: mpsc::Receiver<()>) {
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("worker should release captured resource");
}

#[test]
fn test_hook_recovery_survives_panicking_logger_and_releases_workers() {
    log::set_logger(&LOGGER).expect("isolated logger registration");
    log::set_max_level(LevelFilter::Warn);

    for panic_payload_drop in [false, true] {
        PANIC_WITH_DROP_PAYLOAD.store(panic_payload_drop, Ordering::SeqCst);

        let (direct_hook, direct_finished_rx) = PanickingHook::with_finished_signal();
        let direct_hook = Arc::new(direct_hook);
        let direct = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            DirectExecutor::new()
                .with_hook(direct_hook.clone())
                .call(|| Ok::<_, io::Error>(42))
                .expect("direct task should be accepted")
                .get()
                .expect("direct task should succeed")
        }));
        assert_eq!(direct.expect("hook and logger panics must be contained"), 42);
        direct_finished_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("direct task should finish");
        assert_lifecycle(&direct_hook, "direct");

        let (thread_hook, thread_finished_rx) = PanickingHook::with_finished_signal();
        let thread_hook = Arc::new(thread_hook);
        let (released_tx, released_rx) = mpsc::channel();
        let mut guard = Some(DropSignal(released_tx));
        let thread_task = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            ThreadPerTaskExecutor::new()
                .with_hook(thread_hook.clone())
                .call(move || {
                    drop(guard.take());
                    Ok::<_, io::Error>(42)
                })
                .expect("thread task should be accepted")
                .get()
                .expect("thread task should succeed")
        }));
        assert_eq!(thread_task.expect("thread hook/logger panic must be contained"), 42);
        wait_for_drop(released_rx);
        thread_finished_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("thread task should finish");
        assert_lifecycle(&thread_hook, "thread");

        let (delay_hook, delay_finished_rx) = PanickingHook::with_finished_signal();
        let delay_hook = Arc::new(delay_hook);
        let (released_tx, released_rx) = mpsc::channel();
        let mut guard = Some(DropSignal(released_tx));
        let delay_task = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            DelayExecutor::new(Duration::ZERO)
                .with_hook(delay_hook.clone())
                .call(move || {
                    drop(guard.take());
                    Ok::<_, io::Error>(42)
                })
                .expect("delayed task should be accepted")
                .get()
                .expect("delayed task should succeed")
        }));
        assert_eq!(delay_task.expect("delay hook/logger panic must be contained"), 42);
        wait_for_drop(released_rx);
        delay_finished_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("delayed task should finish");
        assert_lifecycle(&delay_hook, "delay");

        let (schedule_hook, schedule_finished_rx) = PanickingHook::with_finished_signal();
        let schedule_hook = Arc::new(schedule_hook);
        let (released_tx, released_rx) = mpsc::channel();
        let mut guard = Some(DropSignal(released_tx));
        let scheduled_task = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            ScheduleExecutor::at(Instant::now())
                .with_hook(schedule_hook.clone())
                .call(move || {
                    drop(guard.take());
                    Ok::<_, io::Error>(42)
                })
                .expect("scheduled task should be accepted")
                .get()
                .expect("scheduled task should succeed")
        }));
        assert_eq!(
            scheduled_task.expect("schedule hook/logger panic must be contained"),
            42
        );
        wait_for_drop(released_rx);
        schedule_finished_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("scheduled task should finish");
        assert_lifecycle(&schedule_hook, "schedule");

        let rejected_hook = Arc::new(PanickingHook {
            events: Mutex::new(Vec::new()),
            finished: Mutex::new(None),
        });
        let service = ThreadPerTaskExecutorService::builder()
            .hook(rejected_hook.clone())
            .build()
            .expect("service should build");
        service.shutdown();
        let rejected = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            service.submit_callable(|| Ok::<_, io::Error>(42))
        }));
        assert!(matches!(
            rejected.expect("rejection hook/logger panic must be contained"),
            Err(SubmissionError::Shutdown),
        ));
        assert_eq!(rejected_hook.events(), vec!["rejected:Shutdown"]);
    }
}
