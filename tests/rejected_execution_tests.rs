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
    error::Error,
    io,
    sync::Arc,
};

use qubit_executor::service::RejectedExecution;

/// Test rejected execution display, equality, and source behavior.
#[test]
fn test_rejected_execution_variants_display_and_compare() {
    assert_eq!(
        RejectedExecution::Shutdown.to_string(),
        "task rejected because the executor service is shut down",
    );
    assert_eq!(
        RejectedExecution::Saturated.to_string(),
        "task rejected because the executor service is saturated",
    );

    let first = RejectedExecution::WorkerSpawnFailed {
        source: Arc::new(io::Error::other("first")),
    };
    let second = RejectedExecution::WorkerSpawnFailed {
        source: Arc::new(io::Error::other("second")),
    };

    assert_eq!(first, second);
    assert_ne!(first, RejectedExecution::Shutdown);
    assert_eq!(
        first
            .source()
            .expect("worker spawn failure should expose source")
            .to_string(),
        "first",
    );
}
