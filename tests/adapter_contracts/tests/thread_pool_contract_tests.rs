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
use qubit_thread_pool::FixedThreadPool;
use qubit_thread_pool::ThreadPool;

use crate::internal::contract_assertions::assert_concurrent_pending_cancellation;
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

fn blocked_service<S: ExecutorService>(service: S) -> (S, ReleaseGuard) {
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
        .expect("thread pool worker should start blocker");
    (service, ReleaseGuard(Some(release_tx)))
}

fn fixed_pool() -> FixedThreadPool {
    FixedThreadPool::builder()
        .pool_size(1)
        .queue_capacity(8)
        .build()
        .expect("fixed pool should build")
}

fn dynamic_pool() -> ThreadPool {
    ThreadPool::builder()
        .core_pool_size(1)
        .maximum_pool_size(1)
        .queue_capacity(8)
        .build()
        .expect("dynamic pool should build")
}

macro_rules! pool_contract_tests {
    (
        $success:ident, $failure:ident, $shutdown:ident, $cancel:ident, $concurrent_cancel:ident,
        $drop:ident, $drop_polled:ident, $panic_recovery:ident, $pool:ident
    ) => {
        #[test]
        fn $success() {
            let service = $pool();
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
        fn $failure() {
            let service = $pool();
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
        fn $shutdown() {
            let service = $pool();
            assert_shutdown_rejection(&service);
        }

        #[test]
        fn $cancel() {
            let (service, mut release) = blocked_service($pool());
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
        fn $concurrent_cancel() {
            crate::process_watchdog::run_test(
                concat!("thread_pool_contract_tests::", stringify!($concurrent_cancel)),
                concat!(stringify!($concurrent_cancel), "-watchdog"),
                Duration::from_secs(30),
                || {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_time()
                        .build()
                        .expect("test runtime should build");
                    let (service, mut release) = blocked_service($pool());
                    let calls = Arc::new(AtomicUsize::new(0));
                    let task_calls = Arc::clone(&calls);
                    let task = service
                        .submit_tracked_callable(move || {
                            task_calls.fetch_add(1, Ordering::AcqRel);
                            Ok::<usize, io::Error>(42)
                        })
                        .expect("queued task should be accepted");
                    runtime.block_on(assert_concurrent_pending_cancellation(task));
                    assert_eq!(calls.load(Ordering::Acquire), 0);
                    release.release();
                    service.shutdown();
                    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
                    assert_eq!(calls.load(Ordering::Acquire), 0);
                },
            )
            .expect("watchdog should observe thread pool concurrent cancellation test finish");
        }

        #[test]
        fn $drop() {
            let (service, mut release) = blocked_service($pool());
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
        fn $drop_polled() {
            let (service, mut release) = blocked_service($pool());
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

        #[test]
        fn $panic_recovery() {
            let service = $pool();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap();
            runtime.block_on(assert_panic_recovery(&service));
            service.shutdown();
            assert!(service.wait_termination_timeout(Duration::from_secs(5)));
        }
    };
}

pool_contract_tests!(
    test_fixed_successful_callable_returns_its_value,
    test_fixed_callable_failure_preserves_error_kind,
    test_fixed_shutdown_rejects_new_callable,
    test_fixed_pending_callable_can_be_cancelled,
    test_fixed_concurrent_pending_cancellation,
    test_fixed_dropping_future_does_not_cancel_task,
    test_fixed_dropping_polled_future_does_not_cancel_task,
    test_fixed_panic_recovery,
    fixed_pool
);

pool_contract_tests!(
    test_dynamic_successful_callable_returns_its_value,
    test_dynamic_callable_failure_preserves_error_kind,
    test_dynamic_shutdown_rejects_new_callable,
    test_dynamic_pending_callable_can_be_cancelled,
    test_dynamic_concurrent_pending_cancellation,
    test_dynamic_dropping_future_does_not_cancel_task,
    test_dynamic_dropping_polled_future_does_not_cancel_task,
    test_dynamic_panic_recovery,
    dynamic_pool
);
