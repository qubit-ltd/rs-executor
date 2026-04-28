/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
use qubit_function::{
    Callable,
    Runnable,
};

/// Executes fallible one-time tasks according to an implementation-defined strategy.
///
/// `Executor` models an execution strategy, not a managed task service. An
/// executor may run a task immediately, retry it, delay it, schedule it on
/// another runtime, or return a handle that represents work running elsewhere.
/// The associated [`Self::Execution`] type describes how this executor exposes
/// the result of a single execution.
///
/// # Author
///
/// Haixing Hu
pub trait Executor: Send + Sync {
    /// The result carrier returned for one execution.
    ///
    /// Implementations choose the carrier that matches their execution model.
    /// For example, a direct executor can use `Result<R, E>`, while a threaded
    /// executor can use a task handle and a future-backed executor can use a
    /// future.
    type Execution<R, E>
    where
        R: Send + 'static,
        E: std::fmt::Display + Send + 'static;

    /// Executes a runnable task and returns this executor's result carrier.
    ///
    /// This is the unit-returning counterpart of [`Self::call`]. The returned
    /// carrier reports the runnable's `Result<(), E>` according to the concrete
    /// executor's execution model.
    ///
    /// # Parameters
    ///
    /// * `task` - The fallible action to execute.
    ///
    /// # Returns
    ///
    /// The execution carrier for the submitted runnable.
    #[inline]
    fn execute<T, E>(&self, task: T) -> Self::Execution<(), E>
    where
        T: Runnable<E> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        let mut task = task;
        self.call(move || task.run())
    }

    /// Executes a callable task and returns this executor's result carrier.
    ///
    /// # Parameters
    ///
    /// * `task` - The fallible computation to execute.
    ///
    /// # Returns
    ///
    /// The execution carrier for the submitted callable. Its exact behavior is
    /// defined by the concrete executor.
    fn call<C, R, E>(&self, task: C) -> Self::Execution<R, E>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: std::fmt::Display + Send + 'static;
}
