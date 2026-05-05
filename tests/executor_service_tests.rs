/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::io;

use qubit_executor::service::{
    ExecutorService,
    RejectedExecution,
    ThreadPerTaskExecutorService,
};

/// Test default runnable submission and lifecycle rejection through the service trait.
#[test]
fn test_executor_service_submit_default_and_shutdown_rejection() {
    let service = ThreadPerTaskExecutorService::new();

    service
        .submit(|| Ok::<(), io::Error>(()))
        .expect("service should accept runnable")
        .get()
        .expect("submitted runnable should complete");

    service.shutdown();
    let rejected = match service.submit(|| Ok::<(), io::Error>(())) {
        Ok(_) => panic!("shutdown service should reject new runnable"),
        Err(error) => error,
    };

    assert_eq!(rejected, RejectedExecution::Shutdown);
    assert!(service.is_shutdown());
}
