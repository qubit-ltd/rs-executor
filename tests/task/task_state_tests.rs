// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use qubit_executor::task::spi::TaskEndpointPair;

/// Test that `get` returns a completion result when called from another thread.
#[test]
fn test_task_state_returns_completion_result_across_threads() {
    let (handle, completion) = TaskEndpointPair::<usize, io::Error>::new().into_parts();
    let (done_tx, done_rx) = mpsc::channel();
    let waiter = thread::spawn(move || {
        let _ = done_tx.send(handle.get());
    });
    completion.run(|| Ok(42));

    let observed = done_rx.recv_timeout(Duration::from_secs(5));
    let joined = if observed.is_ok() {
        Some(waiter.join())
    } else {
        drop(waiter);
        None
    };
    assert!(matches!(observed, Ok(Ok(42))), "get should return the published value");
    assert!(matches!(joined, Some(Ok(()))), "reader should exit without panic");
}
