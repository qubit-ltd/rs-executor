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
    sync::Arc,
};

use qubit_executor::service::{
    ExecutorBuildError,
    RejectedExecution,
};

/// Tests executor build error display and configuration variants.
#[test]
fn test_executor_build_error_configuration_variants() {
    assert_eq!(
        ExecutorBuildError::ZeroMaximumPoolSize.to_string(),
        "executor service maximum pool size must be greater than zero",
    );
    assert_eq!(
        ExecutorBuildError::CorePoolSizeExceedsMaximum {
            core_pool_size: 4,
            maximum_pool_size: 2,
        }
        .to_string(),
        "executor service core pool size 4 exceeds maximum pool size 2",
    );
    assert_eq!(
        ExecutorBuildError::ZeroQueueCapacity.to_string(),
        "executor service queue capacity must be greater than zero",
    );
    assert_eq!(
        ExecutorBuildError::ZeroStackSize.to_string(),
        "executor service stack size must be greater than zero",
    );
    assert_eq!(
        ExecutorBuildError::ZeroKeepAlive.to_string(),
        "executor service keep-alive timeout must be greater than zero",
    );
}

/// Tests conversion from rejected execution to build error.
#[test]
fn test_executor_build_error_from_rejected_execution() {
    let spawned =
        ExecutorBuildError::from_rejected_execution(RejectedExecution::WorkerSpawnFailed {
            source: Arc::new(io::Error::other("spawn failed")),
        });
    let ExecutorBuildError::SpawnWorker { index, source } = spawned else {
        panic!("worker spawn rejection should convert to spawn build error");
    };
    assert_eq!(index, 0);
    assert_eq!(source.to_string(), "spawn failed");

    let shutdown: ExecutorBuildError = RejectedExecution::Shutdown.into();
    let ExecutorBuildError::SpawnWorker { source, .. } = shutdown else {
        panic!("shutdown during prestart should convert to spawn build error");
    };
    assert_eq!(
        source.to_string(),
        "executor service shut down during prestart"
    );

    let saturated = ExecutorBuildError::from(RejectedExecution::Saturated);
    let ExecutorBuildError::SpawnWorker { source, .. } = saturated else {
        panic!("saturation during prestart should convert to spawn build error");
    };
    assert_eq!(
        source.to_string(),
        "executor service saturated during prestart"
    );
}
