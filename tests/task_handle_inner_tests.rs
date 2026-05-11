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
    thread,
    time::Duration,
};

use qubit_executor::task::TaskCompletionPair;

/// Test shared handle internals wake blocking waiters after completion.
#[test]
fn test_task_handle_inner_notifies_blocking_waiter_on_completion() {
    let (handle, completion) = TaskCompletionPair::<usize, io::Error>::new().into_parts();

    let waiter = thread::spawn(move || handle.get().expect("waiter should receive result"));
    thread::sleep(Duration::from_millis(20));
    completion.complete(Ok(42));

    assert_eq!(waiter.join().expect("waiter thread should not panic"), 42);
}
