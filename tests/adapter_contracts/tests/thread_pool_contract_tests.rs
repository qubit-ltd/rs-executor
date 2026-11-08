// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::future::IntoFuture;
use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use qubit_executor::CancelResult;
use qubit_executor::CancellableTaskHandle;
use qubit_executor::ExecutorService;
use qubit_executor::SubmissionError;
use qubit_executor::TaskExecutionError;
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

struct DropProbe(mpsc::Sender<()>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

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

fn fixed_pool_capacity_one() -> FixedThreadPool {
    FixedThreadPool::builder()
        .pool_size(1)
        .queue_capacity(1)
        .build()
        .expect("fixed pool with capacity one should build")
}

fn dynamic_pool_capacity_one() -> ThreadPool {
    ThreadPool::builder()
        .core_pool_size(1)
        .maximum_pool_size(1)
        .queue_capacity(1)
        .build()
        .expect("dynamic pool with capacity one should build")
}

macro_rules! cancelled_capacity_lifecycle_test {
    ($name:ident, $pool:ident) => {
        #[test]
        fn $name() {
            crate::process_watchdog::run_test(
                concat!("thread_pool_contract_tests::", stringify!($name)),
                concat!(stringify!($name), "-watchdog"),
                Duration::from_secs(60),
                || {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_time()
                        .build()
                        .expect("test runtime should build");
                    let service = $pool();

                    let (started_tx, started_rx) = mpsc::channel();
                    let (release_tx, release_rx) = mpsc::channel();
                    let mut release_a = ReleaseGuard(Some(release_tx));
                    let task_a = service
                        .submit_callable(move || {
                            started_tx.send(()).expect("A start observer should exist");
                            release_rx
                                .recv_timeout(Duration::from_secs(5))
                                .expect("A should be released within five seconds");
                            Ok::<usize, io::Error>(10)
                        })
                        .expect("A should be accepted");
                    started_rx
                        .recv_timeout(Duration::from_secs(5))
                        .expect("A should start on the sole worker");

                    let calls_b = Arc::new(AtomicUsize::new(0));
                    let task_calls_b = Arc::clone(&calls_b);
                    let (dropped_tx, dropped_rx) = mpsc::channel();
                    let probe = DropProbe(dropped_tx);
                    let task_b = service
                        .submit_tracked_callable(move || {
                            let _probe_reference = &probe;
                            task_calls_b.fetch_add(1, Ordering::AcqRel);
                            Ok::<usize, io::Error>(20)
                        })
                        .expect("B should fill the sole queue slot");
                    let (rejected_probe_tx, rejected_probe_rx) = mpsc::channel();
                    let rejected_probe = DropProbe(rejected_probe_tx);
                    assert!(matches!(
                        service.submit_callable(move || {
                            let _probe_reference = &rejected_probe;
                            Ok::<(), io::Error>(())
                        }),
                        Err(SubmissionError::Saturated)
                    ));
                    rejected_probe_rx
                        .recv_timeout(Duration::from_secs(5))
                        .expect("rejected callable capture should be dropped");

                    let cancel_b = CancellableTaskHandle::cancellation_handle(&task_b);
                    assert_eq!(cancel_b.cancel(), CancelResult::Cancelled);
                    assert!(matches!(
                        runtime
                            .block_on(async {
                                tokio::time::timeout(Duration::from_secs(5), task_b.into_future()).await
                            })
                            .expect("B cancellation result should arrive"),
                        Err(TaskExecutionError::Cancelled)
                    ));
                    assert_eq!(calls_b.load(Ordering::Acquire), 0);
                    assert!(
                        dropped_rx.try_recv().is_err(),
                        "B remains queued until worker cleanup"
                    );
                    assert!(matches!(
                        service.submit_callable(|| Ok::<(), io::Error>(())),
                        Err(SubmissionError::Saturated)
                    ));

                    release_a.release();
                    assert_eq!(
                        runtime
                            .block_on(async {
                                tokio::time::timeout(Duration::from_secs(5), task_a.into_future()).await
                            })
                            .expect("A result should arrive")
                            .expect("A should succeed"),
                        10
                    );
                    dropped_rx
                        .recv_timeout(Duration::from_secs(5))
                        .expect("worker should drop B after skipping its cancelled callable");
                    assert_eq!(calls_b.load(Ordering::Acquire), 0);

                    let (started_c_tx, started_c_rx) = mpsc::channel();
                    let (release_c_tx, release_c_rx) = mpsc::channel();
                    let mut release_c = ReleaseGuard(Some(release_c_tx));
                    let task_c = service
                        .submit_callable(move || {
                            started_c_tx.send(()).expect("C start observer should exist");
                            release_c_rx
                                .recv_timeout(Duration::from_secs(5))
                                .expect("C should be released within five seconds");
                            Ok::<usize, io::Error>(30)
                        })
                        .expect("C should be accepted after B cleanup");
                    started_c_rx
                        .recv_timeout(Duration::from_secs(5))
                        .expect("C should start on the sole worker");

                    let calls_d = Arc::new(AtomicUsize::new(0));
                    let task_calls_d = Arc::clone(&calls_d);
                    let task_d = service
                        .submit_callable(move || {
                            task_calls_d.fetch_add(1, Ordering::AcqRel);
                            Ok::<usize, io::Error>(40)
                        })
                        .expect("D should occupy the restored queue slot");
                    assert!(matches!(
                        service.submit_callable(|| Ok::<(), io::Error>(())),
                        Err(SubmissionError::Saturated)
                    ));
                    release_c.release();
                    assert_eq!(
                        runtime
                            .block_on(async {
                                tokio::time::timeout(Duration::from_secs(5), task_c.into_future()).await
                            })
                            .expect("C result should arrive")
                            .expect("C should succeed"),
                        30
                    );
                    assert_eq!(
                        runtime
                            .block_on(async {
                                tokio::time::timeout(Duration::from_secs(5), task_d.into_future()).await
                            })
                            .expect("D result should arrive")
                            .expect("D should succeed"),
                        40
                    );
                    assert_eq!(calls_d.load(Ordering::Acquire), 1);
                    service.shutdown();
                    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
                    assert_eq!(calls_b.load(Ordering::Acquire), 0);
                },
            )
            .expect("watchdog should observe cancelled task capacity lifecycle test finish");
        }
    };
}

cancelled_capacity_lifecycle_test!(test_fixed_cancelled_task_capacity_lifecycle, fixed_pool_capacity_one);
cancelled_capacity_lifecycle_test!(
    test_dynamic_cancelled_task_capacity_lifecycle,
    dynamic_pool_capacity_one
);

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
