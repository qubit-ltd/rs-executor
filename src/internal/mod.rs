// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
mod panic_payload;

#[cfg(test)]
pub(crate) mod test_watchdog_tests;

pub(crate) use panic_payload::discard_panic_payload;
