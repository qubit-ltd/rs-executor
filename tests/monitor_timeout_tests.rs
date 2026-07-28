// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for total-duration monitor waits.

use std::time::Duration;

use qubit_clock::TimeError;
use qubit_executor::wait_until_ready_with_total_timeout;
use qubit_lock::ParkingLotMonitor;

#[test]
/// Verifies a zero total timeout reports a non-ready state without waiting.
fn test_wait_until_ready_with_total_timeout_reports_timeout() {
    let monitor = ParkingLotMonitor::new(false);

    assert!(matches!(
        wait_until_ready_with_total_timeout(
            &monitor,
            Duration::ZERO,
            |ready| *ready,
        ),
        Ok(false),
    ));
}

#[test]
/// Verifies an unrepresentable total timeout reports the Timer error.
fn test_wait_until_ready_with_total_timeout_reports_deadline_overflow() {
    let monitor = ParkingLotMonitor::new(false);

    assert!(matches!(
        wait_until_ready_with_total_timeout(
            &monitor,
            Duration::MAX,
            |ready| *ready,
        ),
        Err(TimeError::InstantOverflow),
    ));
}
