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
use std::sync::Once;

use log::LevelFilter;
use log::Log;
use log::Metadata;
use log::Record;
use log::set_logger;
use log::set_max_level;
use qubit_executor::TaskStatus;
use qubit_executor::executor::DirectExecutor;
use qubit_executor::executor::Executor;
use qubit_executor::executor::ThreadPerTaskExecutor;
use qubit_executor::hook::LoggingTaskHook;
use qubit_executor::hook::TaskHook;
use qubit_executor::hook::TaskId;
use qubit_executor::service::SubmissionError;

struct TestLogger;

impl Log for TestLogger {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, _record: &Record<'_>) {}

    fn flush(&self) {}
}

static LOGGER: TestLogger = TestLogger;
static INIT_LOGGER: Once = Once::new();

fn init_logger() {
    INIT_LOGGER.call_once(|| {
        set_logger(&LOGGER).expect("test logger should install once");
        set_max_level(LevelFilter::Trace);
    });
}

#[derive(Default)]
struct RecordingHook {
    events: Mutex<Vec<String>>,
}

impl RecordingHook {
    fn events(&self) -> Vec<String> {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .clone()
    }
}

struct PanickingHook;

impl TaskHook for PanickingHook {
    fn on_accepted(&self, _task_id: TaskId) {
        panic!("accepted hook panic");
    }

    fn on_started(&self, _task_id: TaskId) {
        panic!("started hook panic");
    }

    fn on_finished(&self, _task_id: TaskId, _status: TaskStatus) {
        panic!("finished hook panic");
    }

    fn on_rejected(&self, _error: &SubmissionError) {
        panic!("rejected hook panic");
    }
}

impl TaskHook for RecordingHook {
    fn on_accepted(&self, task_id: TaskId) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push(format!("accepted:{}", task_id.get()));
    }

    fn on_started(&self, task_id: TaskId) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push(format!("started:{}", task_id.get()));
    }

    fn on_finished(&self, task_id: TaskId, status: TaskStatus) {
        self.events
            .lock()
            .expect("events lock should not be poisoned")
            .push(format!("finished:{}:{status:?}", task_id.get()));
    }
}

#[test]
fn test_task_hook_panics_do_not_break_task_execution() {
    let executor = DirectExecutor::new().with_hook(Arc::new(PanickingHook));

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        executor
            .call(|| Ok::<usize, io::Error>(42))
            .expect("direct executor should still accept task")
            .get()
    }));

    assert_eq!(
        result
            .expect("hook panic should be contained")
            .expect("task should still complete"),
        42,
    );
}

#[test]
fn test_task_hook_observes_direct_executor_lifecycle() {
    let hook = Arc::new(RecordingHook::default());
    let executor = DirectExecutor::new().with_hook(hook.clone());

    let handle = executor
        .call(|| Ok::<usize, io::Error>(42))
        .expect("direct executor should accept task");
    let task_id = handle.task_id().get();
    assert_eq!(handle.get().expect("task should complete"), 42);

    assert_eq!(
        hook.events(),
        vec![
            format!("accepted:{task_id}"),
            format!("started:{task_id}"),
            format!("finished:{task_id}:Succeeded"),
        ],
    );
}

#[test]
fn test_logging_task_hook_is_constructible() {
    init_logger();
    let hook: Arc<dyn TaskHook> = Arc::new(LoggingTaskHook);
    hook.on_accepted(TaskId::new(1));
    hook.on_rejected(&SubmissionError::Shutdown);
    hook.on_started(TaskId::new(1));
    hook.on_finished(TaskId::new(1), TaskStatus::Succeeded);
}

#[test]
fn test_task_hook_rejected_panics_do_not_break_rejection() {
    let executor = ThreadPerTaskExecutor::builder()
        .hook(Arc::new(PanickingHook))
        .stack_size(usize::MAX)
        .build()
        .expect("nonzero stack size should build");

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        executor.call(|| Ok::<usize, io::Error>(42))
    }));

    assert!(matches!(
        result.expect("hook panic should be contained"),
        Err(SubmissionError::WorkerSpawnFailed { .. }),
    ));
}
