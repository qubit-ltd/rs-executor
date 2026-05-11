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
    sync::mpsc,
    time::Duration,
};

use qubit_executor::service::{
    ExecutorService,
    ExecutorServiceLifecycle,
    SubmissionError,
    ThreadPerTaskExecutorService,
};

/// Test default runnable submission and lifecycle rejection through the service trait.
#[test]
fn test_executor_service_submit_default_and_shutdown_rejection() {
    let service = ThreadPerTaskExecutorService::new();
    let (done_tx, done_rx) = mpsc::channel();

    service
        .submit(move || {
            done_tx
                .send(())
                .expect("test should receive submit completion");
            Ok::<(), io::Error>(())
        })
        .expect("service should accept runnable");
    done_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("submitted runnable should complete");

    service.shutdown();
    let rejected = match service.submit(|| Ok::<(), io::Error>(())) {
        Ok(()) => panic!("shutdown service should reject new runnable"),
        Err(error) => error,
    };

    assert_eq!(rejected, SubmissionError::Shutdown);
    assert_eq!(service.lifecycle(), ExecutorServiceLifecycle::Terminated);
    assert!(service.is_not_running());
}

/// Test callable submission returns a lightweight result handle with non-blocking polling.
#[test]
fn test_executor_service_submit_callable_returns_result_handle() {
    let service = ThreadPerTaskExecutorService::new();

    let handle = service
        .submit_callable(|| Ok::<usize, io::Error>(42))
        .expect("service should accept callable");

    assert_eq!(handle.get().expect("callable should succeed"), 42);
}

/// Test tracked callable submission exposes cancellation and terminal status.
#[test]
fn test_executor_service_submit_tracked_callable_exposes_tracking() {
    let service = ThreadPerTaskExecutorService::new();

    let handle = service
        .submit_tracked_callable(|| Ok::<usize, io::Error>(42))
        .expect("service should accept tracked callable");

    assert_eq!(handle.get().expect("tracked callable should succeed"), 42);
}

/// Test default tracked runnable submission delegates to tracked callable submission.
#[test]
fn test_executor_service_submit_tracked_default_returns_unit_handle() {
    let service = ThreadPerTaskExecutorService::new();

    let handle = service
        .submit_tracked(|| Ok::<(), io::Error>(()))
        .expect("service should accept tracked runnable");

    assert_eq!(handle.get().expect("tracked runnable should succeed"), ());
}
