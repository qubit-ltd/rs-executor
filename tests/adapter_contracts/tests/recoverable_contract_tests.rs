// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::io;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use qubit_executor::ExecutorService;
use qubit_executor::RecoverableExecutorService;
use qubit_executor::SubmissionError;
use qubit_rayon_executor::RayonExecutorService;
use qubit_tokio_executor::TokioExecutorService;
use tokio::runtime::Runtime;

struct DropCount(Arc<AtomicUsize>);
impl Drop for DropCount {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
}

fn saturation_retry<S>(service: &S, release: mpsc::Sender<()>, done: mpsc::Receiver<()>)
where
    S: RecoverableExecutorService,
    S::ResultHandle<usize, io::Error>: std::future::IntoFuture<Output = qubit_executor::TaskResult<usize, io::Error>>,
    <S::ResultHandle<usize, io::Error> as std::future::IntoFuture>::IntoFuture: std::future::Future,
{
    let calls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let task_calls = Arc::clone(&calls);
    let probe = DropCount(Arc::clone(&drops));
    let task = move || {
        let _probe = &probe;
        task_calls.fetch_add(1, Ordering::AcqRel);
        Ok::<usize, io::Error>(42)
    };
    let rejected = match service.try_submit_callable(task) {
        Err(rejected) => rejected,
        Ok(_) => panic!("capacity should be full"),
    };
    let (reason, task) = rejected.into_parts();
    assert_eq!(reason, SubmissionError::Saturated);
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert_eq!(drops.load(Ordering::Acquire), 0);
    release.send(()).expect("blocker should receive release");
    done.recv_timeout(Duration::from_secs(5))
        .expect("blocker should finish before retry");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut task = Some(task);
    let handle = loop {
        match service.try_submit_callable(task.take().expect("callable should be retained")) {
            Ok(handle) => break handle,
            Err(rejected) => {
                let (reason, returned) = rejected.into_parts();
                assert_eq!(
                    reason,
                    SubmissionError::Saturated,
                    "only transient saturation is retried"
                );
                assert!(
                    std::time::Instant::now() < deadline,
                    "released capacity should become available"
                );
                task = Some(returned);
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    assert_eq!(
        runtime
            .block_on(std::future::IntoFuture::into_future(handle))
            .expect("retried task should succeed"),
        42
    );
    assert_eq!(calls.load(Ordering::Acquire), 1);
    assert_eq!(drops.load(Ordering::Acquire), 1);
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}

fn shutdown_recovery<S: RecoverableExecutorService>(service: &S) {
    service.shutdown();
    let drops = Arc::new(AtomicUsize::new(0));
    let probe = DropCount(Arc::clone(&drops));
    let rejected = match service.try_submit_callable(move || {
        let _ = &probe;
        Ok::<usize, io::Error>(42)
    }) {
        Err(rejected) => rejected,
        Ok(_) => panic!("shutdown service should reject"),
    };
    let (reason, task) = rejected.into_parts();
    assert_eq!(reason, SubmissionError::Shutdown);
    assert_eq!(drops.load(Ordering::Acquire), 0);
    drop(task);
    assert_eq!(drops.load(Ordering::Acquire), 1);
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}

fn rayon() -> RayonExecutorService {
    RayonExecutorService::builder()
        .num_threads(1)
        .task_capacity(1)
        .build()
        .unwrap()
}
fn runtime() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap()
}
fn tokio_service(runtime: &Runtime) -> TokioExecutorService {
    TokioExecutorService::with_task_capacity(runtime.handle().clone(), NonZeroUsize::new(1).unwrap())
}

#[test]
fn test_native_services_implement_recoverable_contract() {
    fn requires<S: RecoverableExecutorService>() {}
    requires::<RayonExecutorService>();
    requires::<TokioExecutorService>();
}

#[test]
fn test_rayon_recoverable_callable_can_be_retried_after_saturation() {
    crate::process_watchdog::run_test(
        "recoverable_contract_tests::test_rayon_recoverable_callable_can_be_retried_after_saturation",
        "recoverable-rayon-saturation-retry",
        Duration::from_secs(30),
        || {
            let service = rayon();
            let (started_tx, started_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let (done_tx, done_rx) = mpsc::channel();
            service
                .submit_callable(move || {
                    started_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(20)).unwrap();
                    done_tx.send(()).unwrap();
                    Ok::<(), io::Error>(())
                })
                .unwrap();
            started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            saturation_retry(&service, release_tx, done_rx);
        },
    )
    .expect("watchdog should observe Rayon recoverable saturation retry");
}

#[test]
fn test_tokio_recoverable_callable_can_be_retried_after_saturation() {
    crate::process_watchdog::run_test(
        "recoverable_contract_tests::test_tokio_recoverable_callable_can_be_retried_after_saturation",
        "recoverable-tokio-saturation-retry",
        Duration::from_secs(30),
        || {
            let runtime = runtime();
            let service = tokio_service(&runtime);
            let (started_tx, started_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let (done_tx, done_rx) = mpsc::channel();
            service
                .submit_callable(move || {
                    started_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(20)).unwrap();
                    done_tx.send(()).unwrap();
                    Ok::<(), io::Error>(())
                })
                .unwrap();
            started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            saturation_retry(&service, release_tx, done_rx);
        },
    )
    .expect("watchdog should observe Tokio recoverable saturation retry");
}

#[test]
fn test_rayon_recoverable_shutdown_returns_callable() {
    crate::process_watchdog::run_test(
        "recoverable_contract_tests::test_rayon_recoverable_shutdown_returns_callable",
        "recoverable-rayon-shutdown-recovery",
        Duration::from_secs(30),
        || shutdown_recovery(&rayon()),
    )
    .expect("watchdog should observe Rayon recoverable shutdown rejection");
}
#[test]
fn test_tokio_recoverable_shutdown_returns_callable() {
    crate::process_watchdog::run_test(
        "recoverable_contract_tests::test_tokio_recoverable_shutdown_returns_callable",
        "recoverable-tokio-shutdown-recovery",
        Duration::from_secs(30),
        || {
            let runtime = runtime();
            shutdown_recovery(&tokio_service(&runtime));
        },
    )
    .expect("watchdog should observe Tokio recoverable shutdown rejection");
}

#[test]
fn test_legacy_service_does_not_need_recoverable_extension() {
    fn accepts_legacy<S: ExecutorService>(_: &S) {}
    let service = RayonExecutorService::builder().num_threads(1).build().unwrap();
    accepts_legacy(&service);
    service.shutdown();
    assert!(service.wait_termination_timeout(Duration::from_secs(5)));
}
