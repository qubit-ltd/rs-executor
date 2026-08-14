// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::io;

use qubit_executor::CancelResult;
use qubit_executor::TaskExecutionError;
use qubit_executor::task::spi::TaskEndpointPair;
use qubit_executor::task::spi::TaskSlotCell;

/// Verifies that a failed start returns ownership to the shared slot cell.
#[test]
fn test_task_slot_cell_try_start_restores_cancelled_slot() {
    let (handle, slot) =
        TaskEndpointPair::<usize, io::Error>::new().into_tracked_parts();
    let cell = TaskSlotCell::new(slot);
    cell.accept();
    assert_eq!(handle.cancel(), CancelResult::Cancelled);

    assert!(cell.try_start().is_none());
    assert!(cell.take().is_some());
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
}

/// Verifies that shared cancellation consumes an unstarted task only once.
#[test]
fn test_task_slot_cell_cancel_unstarted_publishes_cancellation() {
    let (handle, slot) =
        TaskEndpointPair::<usize, io::Error>::new().into_tracked_parts();
    let cell = TaskSlotCell::new(slot);
    cell.accept();

    assert!(cell.cancel_unstarted());
    assert!(!cell.cancel_unstarted());
    assert!(matches!(handle.get(), Err(TaskExecutionError::Cancelled)));
}
