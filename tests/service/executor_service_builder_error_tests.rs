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
    ExecutorServiceBuilderError,
    SubmissionError,
};

/// Tests executor build error display and configuration variants.
#[test]
fn test_executor_service_builder_error_configuration_variants() {
    assert_eq!(
        ExecutorServiceBuilderError::ZeroMaximumPoolSize.to_string(),
        "executor service maximum pool size must be greater than zero",
    );
    assert_eq!(
        ExecutorServiceBuilderError::CorePoolSizeExceedsMaximum {
            core_pool_size: 4,
            maximum_pool_size: 2,
        }
        .to_string(),
        "executor service core pool size 4 exceeds maximum pool size 2",
    );
    assert_eq!(
        ExecutorServiceBuilderError::ZeroQueueCapacity.to_string(),
        "executor service queue capacity must be greater than zero",
    );
    assert_eq!(
        ExecutorServiceBuilderError::ZeroStackSize.to_string(),
        "executor service stack size must be greater than zero",
    );
    assert_eq!(
        ExecutorServiceBuilderError::ZeroKeepAlive.to_string(),
        "executor service keep-alive timeout must be greater than zero",
    );
}

/// Tests conversion from rejected execution to build error.
#[test]
fn test_executor_service_builder_error_from_submission_error() {
    let spawned =
        ExecutorServiceBuilderError::from_submission_error(SubmissionError::WorkerSpawnFailed {
            source: Arc::new(io::Error::other("spawn failed")),
        });
    assert_eq!(
        spawned.to_string(),
        "failed to spawn executor service worker unknown: spawn failed",
    );
    let ExecutorServiceBuilderError::SpawnWorker { index, source } = spawned else {
        panic!("worker spawn rejection should convert to spawn build error");
    };
    assert_eq!(index, None);
    assert_eq!(source.to_string(), "spawn failed");
    assert_eq!(
        ExecutorServiceBuilderError::SpawnWorker {
            index: Some(7),
            source: io::Error::other("indexed spawn failed"),
        }
        .to_string(),
        "failed to spawn executor service worker 7: indexed spawn failed",
    );

    let shutdown: ExecutorServiceBuilderError = SubmissionError::Shutdown.into();
    let ExecutorServiceBuilderError::SpawnWorker { source, .. } = shutdown else {
        panic!("shutdown during prestart should convert to spawn build error");
    };
    assert_eq!(
        source.to_string(),
        "executor service shut down during prestart"
    );

    let saturated = ExecutorServiceBuilderError::from(SubmissionError::Saturated);
    let ExecutorServiceBuilderError::SpawnWorker { source, .. } = saturated else {
        panic!("saturation during prestart should convert to spawn build error");
    };
    assert_eq!(
        source.to_string(),
        "executor service saturated during prestart"
    );
}
