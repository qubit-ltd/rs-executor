// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::thread;
use std::time::Duration;

use super::process_watchdog;

#[test]
fn test_process_watchdog_accepts_successful_child() {
    process_watchdog::run_test(
        "internal::process_watchdog_tests::test_process_watchdog_accepts_successful_child",
        "watchdog-success",
        process_watchdog::DEFAULT_TIMEOUT,
        || {},
    )
    .expect("watchdog should accept a successful child");
}

#[test]
fn test_process_watchdog_reports_nonzero_child_exit() {
    let result = process_watchdog::run_test(
        "internal::process_watchdog_tests::test_process_watchdog_reports_nonzero_child_exit",
        "watchdog-nonzero",
        process_watchdog::DEFAULT_TIMEOUT,
        || panic!("intentional watchdog child failure"),
    );
    if let Err(error) = result {
        assert!(error.contains("exited with"), "unexpected watchdog error: {error}");
        assert!(
            error.contains("intentional watchdog child failure"),
            "child output missing from: {error}"
        );
    } else {
        panic!("watchdog should report nonzero child exit");
    }
}

#[test]
fn test_process_watchdog_terminates_hung_child() {
    let result = process_watchdog::run_test(
        "internal::process_watchdog_tests::test_process_watchdog_terminates_hung_child",
        "watchdog-hang",
        Duration::from_millis(250),
        || loop {
            thread::sleep(Duration::from_secs(1));
        },
    );
    if let Err(error) = result {
        assert!(error.contains("exceeded"), "unexpected watchdog error: {error}");
        assert!(error.contains("watchdog-hang"), "child output missing from: {error}");
    } else {
        panic!("watchdog should terminate a hung child");
    }
}
