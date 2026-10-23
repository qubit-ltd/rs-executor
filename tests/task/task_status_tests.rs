// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_executor::TaskStatus;
use qubit_state_machine::DenseCode;

#[test]
fn test_task_status_codebook_preserves_discriminants() {
    let expected = [
        TaskStatus::Pending,
        TaskStatus::Running,
        TaskStatus::Succeeded,
        TaskStatus::Failed,
        TaskStatus::Panicked,
        TaskStatus::Cancelled,
        TaskStatus::Dropped,
    ];
    assert_eq!(TaskStatus::VALUES, expected);
    for (index, &value) in TaskStatus::VALUES.iter().enumerate() {
        assert_eq!(value.code(), index as u64);
    }
}
