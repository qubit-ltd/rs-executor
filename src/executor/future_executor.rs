/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
use super::Executor;

/// Marker trait for executors whose execution carrier is a future.
///
/// A `FutureExecutor` is still an [`Executor`]: it executes
/// [`Runnable`](qubit_function::Runnable) and
/// [`Callable`](qubit_function::Callable) tasks through [`Executor::execute`]
/// and [`Executor::call`].
/// Its distinguishing contract is that `Self::Execution<R, E>` should be a
/// future resolving to `Result<R, E>`.
///
/// Rust cannot currently express this contract for all `R` and `E` directly in
/// the trait definition, so implementations must document and uphold it.
///
/// # Author
///
/// Haixing Hu
pub trait FutureExecutor: Executor {}
