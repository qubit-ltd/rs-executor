// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use qubit_executor::ExecutorService;
use qubit_rayon_executor::RayonExecutorService;

use crate::internal::contract_assertions::assert_failure;
use crate::internal::contract_assertions::assert_panic_recovery;
use crate::internal::contract_assertions::assert_pending_cancellation;
use crate::internal::contract_assertions::assert_runs_after_future_drop;
use crate::internal::contract_assertions::assert_runs_after_polled_future_drop;
use crate::internal::contract_assertions::assert_shutdown_rejection;
use crate::internal::contract_assertions::assert_success;

struct ReleaseGuard(Option<mpsc::Sender<()>>);

impl ReleaseGuard {
    fn release(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

impl Drop for ReleaseGuard {
    fn drop(&mut self) {
        self.release();
    }
}

fn blocked_service() -> (RayonExecutorService, ReleaseGuard, mpsc::Receiver<()>) {
    let service = RayonExecutorService::builder()
        .num_threads(1)
        .build()
        .expect("Rayon service should build");
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    service
        .submit_callable(move || {
            started_tx.send(()).expect("blocker observer should exist");
            release_rx
                .recv_timeout(Duration::from_secs(30))
                .expect("blocker should be released within thirty seconds");
            Ok::<(), io::Error>(())
        })
        .expect("blocker should be accepted");
    started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Rayon worker should start blocker");
    (service, ReleaseGuard(Some(release_tx)), started_rx)
}

#[test]
fn test_rayon_successful_callable_returns_its_value() {
    let service = RayonExecutorService::builder().num_threads(1).build().unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let task = service.submit_callable(|| Ok::<usize, io::Error>(42)).unwrap();
    runtime.block_on(assert_success(task));
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}

#[test]
fn test_rayon_panic_recovery() {
    let service = RayonExecutorService::builder().num_threads(1).build().unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(assert_panic_recovery(&service));
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}

#[test]
fn test_rayon_callable_failure_preserves_error_kind() {
    let service = RayonExecutorService::builder().num_threads(1).build().unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let task = service
        .submit_callable(|| Err::<usize, io::Error>(io::Error::from(io::ErrorKind::InvalidInput)))
        .unwrap();
    runtime.block_on(assert_failure(task));
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}

#[test]
fn test_rayon_shutdown_rejects_new_callable() {
    let service = RayonExecutorService::builder().num_threads(1).build().unwrap();
    assert_shutdown_rejection(&service);
}

#[test]
fn test_rayon_pending_callable_can_be_cancelled() {
    let (service, mut release, _) = blocked_service();
    let calls = Arc::new(AtomicUsize::new(0));
    let task_calls = Arc::clone(&calls);
    let task = service
        .submit_tracked_callable(move || {
            task_calls.fetch_add(1, Ordering::AcqRel);
            Ok::<usize, io::Error>(42)
        })
        .expect("queued task should be accepted");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(assert_pending_cancellation(task));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    release.release();
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
    assert_eq!(calls.load(Ordering::Acquire), 0);
}

#[test]
fn test_rayon_dropping_future_does_not_cancel_task() {
    let (service, mut release, _) = blocked_service();
    let calls = Arc::new(AtomicUsize::new(0));
    let task_calls = Arc::clone(&calls);
    let (ran_tx, ran_rx) = mpsc::channel();
    let task = service
        .submit_tracked_callable(move || {
            task_calls.fetch_add(1, Ordering::AcqRel);
            ran_tx.send(()).expect("execution observer should exist");
            Ok::<usize, io::Error>(42)
        })
        .expect("queued task should be accepted");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(assert_runs_after_future_drop(
        task,
        || release.release(),
        || {
            ran_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("dropped future task should still run")
        },
    ));
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
    assert_eq!(calls.load(Ordering::Acquire), 1);
}
#[test]
fn test_rayon_dropping_polled_future_does_not_cancel_task() {
    let (service, mut release, _) = blocked_service();
    let calls = Arc::new(AtomicUsize::new(0));
    let task_calls = Arc::clone(&calls);
    let (ran_tx, ran_rx) = mpsc::channel();
    let task = service
        .submit_tracked_callable(move || {
            task_calls.fetch_add(1, Ordering::AcqRel);
            ran_tx.send(()).expect("execution observer should exist");
            Ok::<usize, io::Error>(42)
        })
        .expect("queued task should be accepted");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(assert_runs_after_polled_future_drop(
        task,
        || release.release(),
        || {
            ran_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("dropped polled future task should still run")
        },
    ));
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
    assert_eq!(calls.load(Ordering::Acquire), 1);
}
