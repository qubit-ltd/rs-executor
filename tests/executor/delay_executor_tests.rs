/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for [`DelayExecutor`](qubit_executor::executor::DelayExecutor).

use std::{
    io,
    sync::mpsc,
    thread,
    time::{
        Duration,
        Instant,
    },
};

use qubit_executor::{
    TaskExecutionError,
    executor::{
        DelayExecutor,
        Executor,
    },
};

fn delayed_value_task() -> Result<usize, io::Error> {
    Ok(42)
}

#[test]
fn test_delay_executor_delays_task_start() {
    let executor = DelayExecutor::new(Duration::from_millis(80));
    let (started_tx, started_rx) = mpsc::channel();
    let start = Instant::now();

    assert_eq!(executor.delay(), Duration::from_millis(80));
    let handle = executor.execute(move || {
        started_tx
            .send(Instant::now())
            .expect("test should receive start time");
        Ok::<(), io::Error>(())
    });

    assert!(
        started_rx.recv_timeout(Duration::from_millis(30)).is_err(),
        "task should not start before the configured delay",
    );
    let started_at = started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("task should start after delay");
    handle.get().expect("delayed task should complete");
    assert!(started_at.duration_since(start) >= Duration::from_millis(70));
}

#[test]
fn test_delay_executor_returns_callable_value() {
    let executor = DelayExecutor::new(Duration::ZERO);

    let handle = executor.call(delayed_value_task as fn() -> Result<usize, io::Error>);

    assert_eq!(handle.get().expect("callable should complete"), 42);
}

#[test]
fn test_delay_executor_cancel_before_start_skips_callable() {
    let executor = DelayExecutor::new(Duration::from_millis(80));

    let handle = executor.call(delayed_value_task as fn() -> Result<usize, io::Error>);

    assert!(handle.cancel());
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
    thread::sleep(Duration::from_millis(120));
}
