/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! # Qubit Executor
//!
//! Core executor abstractions, task handles, and basic executor implementations.
//!

pub mod executor;
pub mod service;
mod task;

pub use crate::task::{
    TaskCompletion,
    TaskCompletionPair,
    TaskHandle,
    TaskRunner,
};
pub use crate::task::{
    TaskExecutionError,
    TaskResult,
};
