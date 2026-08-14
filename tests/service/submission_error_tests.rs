// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::error::Error;
use std::io;
use std::sync::Arc;

use qubit_executor::service::SubmissionError;

/// Test rejected execution display, equality, and source behavior.
#[test]
fn test_submission_error_variants_display_and_compare() {
    assert_eq!(
        SubmissionError::Shutdown.to_string(),
        "task rejected because the executor service is shut down",
    );
    assert_eq!(
        SubmissionError::Saturated.to_string(),
        "task rejected because the executor service is saturated",
    );

    let first = SubmissionError::WorkerSpawnFailed {
        source: Arc::new(io::Error::other("first")),
    };
    let second = SubmissionError::WorkerSpawnFailed {
        source: Arc::new(io::Error::other("second")),
    };

    assert_eq!(first, second);
    assert_ne!(first, SubmissionError::Shutdown);
    assert_eq!(
        first
            .source()
            .expect("worker spawn failure should expose source")
            .to_string(),
        "first",
    );

    let created =
        SubmissionError::worker_spawn_failed(io::Error::other("created"));
    assert!(matches!(created, SubmissionError::WorkerSpawnFailed { .. }));
}
