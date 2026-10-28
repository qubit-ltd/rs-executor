// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::error::Error;
use std::fmt;
use std::io;
use std::sync::Arc;

use qubit_executor::service::ExecutorServiceBuilderError;
use qubit_executor::service::SubmissionError;

/// Tests executor build error display and configuration variants.
#[test]
fn test_executor_service_builder_error_configuration_variants() {
    assert_eq!(
        ExecutorServiceBuilderError::ZeroPoolSize.to_string(),
        "executor service pool size must be greater than zero",
    );
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
    let spawned = ExecutorServiceBuilderError::from_submission_error(SubmissionError::WorkerSpawnFailed {
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
    assert_eq!(source.to_string(), "executor service shut down during prestart");

    let saturated = ExecutorServiceBuilderError::from(SubmissionError::Saturated);
    let ExecutorServiceBuilderError::SpawnWorker { source, .. } = saturated else {
        panic!("saturation during prestart should convert to spawn build error");
    };
    assert_eq!(source.to_string(), "executor service saturated during prestart");
}

#[test]
fn test_executor_service_builder_error_preserves_spawn_io_error_context() {
    for shared in [false, true] {
        let original = Arc::new(io::Error::from_raw_os_error(17));
        let original_kind = original.kind();
        let original_errno = original.raw_os_error();
        let keep_alive = shared.then(|| Arc::clone(&original));
        let converted =
            ExecutorServiceBuilderError::from_submission_error(SubmissionError::WorkerSpawnFailed { source: original });
        let ExecutorServiceBuilderError::SpawnWorker { source, .. } = converted else {
            panic!("spawn failure should retain its variant");
        };
        assert_eq!(source.kind(), original_kind);
        assert_eq!(source.raw_os_error(), original_errno);
        drop(keep_alive);
    }

    #[derive(Debug)]
    struct Nested;
    impl fmt::Display for Nested {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("nested")
        }
    }
    impl Error for Nested {}

    #[derive(Debug)]
    struct Payload;
    impl fmt::Display for Payload {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("payload")
        }
    }
    impl Error for Payload {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&Nested)
        }
    }

    let unique = ExecutorServiceBuilderError::from_submission_error(SubmissionError::WorkerSpawnFailed {
        source: Arc::new(io::Error::other(Payload)),
    });
    let ExecutorServiceBuilderError::SpawnWorker { source, .. } = unique else {
        unreachable!()
    };
    assert!(source.get_ref().and_then(|e| e.downcast_ref::<Payload>()).is_some());
    assert!(source.get_ref().and_then(Error::source).unwrap().is::<Nested>());

    let shared_source = Arc::new(io::Error::other(Payload));
    let keep_alive = Arc::clone(&shared_source);
    let shared = ExecutorServiceBuilderError::from_submission_error(SubmissionError::WorkerSpawnFailed {
        source: shared_source,
    });
    let ExecutorServiceBuilderError::SpawnWorker { source, .. } = shared else {
        unreachable!()
    };
    let wrapped_io = Error::source(&source)
        .and_then(|e| e.downcast_ref::<io::Error>())
        .unwrap();
    assert!(wrapped_io.get_ref().and_then(|e| e.downcast_ref::<Payload>()).is_some());
    assert!(wrapped_io.get_ref().and_then(Error::source).unwrap().is::<Nested>());
    drop(keep_alive);
}
