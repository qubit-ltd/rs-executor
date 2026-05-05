/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use qubit_executor::service::ShutdownReport;

/// Test shutdown report construction and derived value semantics.
#[test]
fn test_shutdown_report_new_default_and_equality() {
    let empty = ShutdownReport::default();
    assert_eq!(empty, ShutdownReport::new(0, 0, 0));

    let report = ShutdownReport::new(1, 2, 3);
    assert_eq!(report.queued, 1);
    assert_eq!(report.running, 2);
    assert_eq!(report.cancelled, 3);
    assert_eq!(report, report);
}
