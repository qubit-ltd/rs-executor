// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! A minimal custom executor backed by an application-owned worker queue.
//!
//! This example omits lifecycle hooks, so it accepts the slot on the worker
//! after returning the handle. An executor with hooks must finish
//! `slot.accept()` before publishing a cancellable handle or allowing a worker
//! to start the task.

use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread;

use qubit_executor::Executor;
use qubit_executor::TrackedTask;
use qubit_executor::service::SubmissionError;
use qubit_executor::task::spi::TaskEndpointPair;
use qubit_executor::task::spi::TaskRunner;
use qubit_function::Callable;

type Job = Box<dyn FnOnce() + Send + 'static>;

struct CustomExecutor {
    sender: Sender<Job>,
}

impl Executor for CustomExecutor {
    fn call<C, R, E>(&self, task: C) -> Result<TrackedTask<R, E>, SubmissionError>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
    {
        let (handle, slot) = TaskEndpointPair::new().into_tracked_parts();
        let job: Job = Box::new(move || {
            slot.accept();
            TaskRunner::new(task).run(slot);
        });
        self.sender.send(job).map_err(|_| SubmissionError::Shutdown)?;
        Ok(handle)
    }
}

fn main() {
    let (sender, receiver) = mpsc::channel::<Job>();
    let worker = thread::spawn(move || {
        for job in receiver {
            job();
        }
    });
    let executor = CustomExecutor { sender };

    let task = executor
        .call(|| Ok::<usize, ()>(42))
        .expect("open queue should accept the task");
    assert_eq!(task.get().expect("task should succeed"), 42);

    drop(executor);
    worker.join().expect("worker should exit after the queue closes");

    let (sender, receiver) = mpsc::channel::<Job>();
    drop(receiver);
    let executor = CustomExecutor { sender };
    assert!(matches!(
        executor.call(|| Ok::<usize, ()>(42)),
        Err(SubmissionError::Shutdown)
    ));
}
