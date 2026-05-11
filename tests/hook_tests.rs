/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::{
    io,
    sync::{
        Arc,
        Mutex,
    },
};

use qubit_executor::{
    TaskStatus,
    executor::{
        DirectExecutor,
        Executor,
    },
    hook::{
        LoggingTaskHook,
        TaskHook,
        TaskId,
    },
};

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
    let hook: Arc<dyn TaskHook> = Arc::new(LoggingTaskHook);
    hook.on_accepted(TaskId::new(1));
}
