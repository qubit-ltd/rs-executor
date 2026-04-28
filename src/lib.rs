/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! # Qubit Executor
//!
//! Core executor abstractions, task handles, and basic executor implementations.
//!
//! # Author
//!
//! Haixing Hu

pub mod executor;
pub mod service;
mod task;

#[doc(hidden)]
pub use crate::task::task_runner;
pub use crate::task::{
    TaskCompletion,
    TaskCompletionPair,
    TaskHandle,
};
pub use crate::task::{
    TaskExecutionError,
    TaskResult,
};
