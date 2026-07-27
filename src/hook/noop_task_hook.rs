// =============================================================================
// qubit-style: allow source-test-pair
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use crate::hook::TaskHook;

/// Task hook that ignores all events.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NoopTaskHook;

impl TaskHook for NoopTaskHook {}
