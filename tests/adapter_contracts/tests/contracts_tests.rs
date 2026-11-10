// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::time::Duration;

mod internal;
mod rayon_contract_tests;
mod thread_pool_contract_tests;
mod tokio_contract_tests;

mod process_watchdog;
mod recoverable_contract_tests;

#[test]
fn test_process_watchdog_default_timeout_is_fifteen_seconds() {
    assert_eq!(process_watchdog::DEFAULT_TIMEOUT, Duration::from_secs(15));
}
