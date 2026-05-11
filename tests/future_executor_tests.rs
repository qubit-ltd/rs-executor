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
    future::{Ready, ready},
    io,
};

use qubit_executor::executor::{Executor, FutureExecutor};
use qubit_function::Callable;

#[derive(Debug, Default)]
struct ReadyExecutor;

impl Executor for ReadyExecutor {
    type Execution<R, E>
        = Ready<Result<R, E>>
    where
        R: Send + 'static,
        E: std::fmt::Display + Send + 'static;

    fn call<C, R, E>(&self, mut task: C) -> Self::Execution<R, E>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        ready(task.call())
    }
}

impl FutureExecutor for ReadyExecutor {}

fn accepts_future_executor<E>(executor: &E) -> Ready<Result<usize, io::Error>>
where
    E: FutureExecutor<Execution<usize, io::Error> = Ready<Result<usize, io::Error>>>,
{
    executor.call(|| Ok::<usize, io::Error>(42))
}

/// Test that a future-backed executor can satisfy the marker trait contract.
#[tokio::test]
async fn test_future_executor_marker_accepts_ready_future_execution() {
    let executor = ReadyExecutor;

    let result = accepts_future_executor(&executor)
        .await
        .expect("ready executor should resolve successfully");

    assert_eq!(result, 42);
}
