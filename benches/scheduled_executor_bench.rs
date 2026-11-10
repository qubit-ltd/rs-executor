// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;

mod statistics;

use qubit_executor::CancelResult;
use qubit_executor::ExecutorService;
use qubit_executor::ScheduledExecutorService;
use qubit_executor::SingleThreadScheduledExecutorService;

const TASKS: usize = 10_000;
const TIMER_DELAY: Duration = Duration::from_millis(5);
const SENTINEL_DELAY: Duration = Duration::from_secs(60);
const DEFAULT_TIMER_SAMPLES: usize = 64;
const CANCEL_TASKS: usize = 1_000;
const START_TIMEOUT: Duration = Duration::from_secs(5);

fn main() {
    let rounds = std::env::var("SCHEDULED_BENCH_ROUNDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(15);
    assert!(rounds > 0);
    let timer_samples = positive_env("SCHEDULED_BENCH_TIMER_SAMPLES", DEFAULT_TIMER_SAMPLES);
    println!(
        "scheduled_executor_bench,tasks={TASKS},rounds={rounds},warmup=1,timer_samples={timer_samples},timer_samples_unit=count,deadline_delay_ns={},sentinel_delay_ns={},cancel_tasks={CANCEL_TASKS}",
        TIMER_DELAY.as_nanos(),
        SENTINEL_DELAY.as_nanos()
    );
    run_workload("sequential", rounds, sequential_round);
    run_workload("batch", rounds, batch_round);
    run_timer_workload("future_deadline", rounds, timer_samples, false);
    run_timer_workload("earlier_deadline", rounds, timer_samples, true);
    run_cancel_workload(rounds);
}

fn positive_env(name: &str, default: usize) -> usize {
    match std::env::var(name) {
        Ok(value) => match value.parse::<usize>() {
            Ok(parsed) if parsed > 0 => parsed,
            _ => panic!("{name} must be a positive integer, got {value:?}"),
        },
        Err(std::env::VarError::NotPresent) => default,
        Err(error) => panic!("failed to read {name}: {error}"),
    }
}

fn run_workload(
    name: &str,
    rounds: usize,
    mut round: impl FnMut(&SingleThreadScheduledExecutorService) -> (Duration, Vec<Duration>),
) {
    let service =
        SingleThreadScheduledExecutorService::new("scheduled-bench-warmup").expect("warmup scheduler should start");
    black_box(round(&service));
    service.shutdown();
    service.wait_termination();

    let mut totals = Vec::with_capacity(rounds);
    let mut p50s = Vec::with_capacity(rounds);
    let mut p95s = Vec::with_capacity(rounds);
    for index in 0..rounds {
        let service = SingleThreadScheduledExecutorService::new("scheduled-bench").expect("scheduler should start");
        let (total, mut latency) = round(&service);
        latency.sort_unstable();
        let p50 = latency[statistics::nearest_rank_index(latency.len(), 50)];
        let p95 = latency[statistics::nearest_rank_index(latency.len(), 95)];
        println!(
            "sample,{name},{index},{},{},{}",
            total.as_nanos(),
            p50.as_nanos(),
            p95.as_nanos()
        );
        totals.push(total);
        p50s.push(p50);
        p95s.push(p95);
        service.shutdown();
        service.wait_termination();
    }
    totals.sort_unstable();
    p50s.sort_unstable();
    p95s.sort_unstable();
    println!(
        "summary,{name},median_total_ns={},median_task_p50_ns={},median_task_p95_ns={}",
        statistics::median_ns(&totals),
        statistics::median_ns(&p50s),
        statistics::median_ns(&p95s)
    );
}

struct StopOnDrop<'a>(&'a SingleThreadScheduledExecutorService);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        let _ = self.0.stop();
    }
}

fn run_timer_workload(name: &str, rounds: usize, samples: usize, insert_earlier: bool) {
    let warmup =
        SingleThreadScheduledExecutorService::new("scheduled-timer-warmup").expect("warmup scheduler should start");
    let warmup_guard = StopOnDrop(&warmup);
    black_box(timer_round(&warmup, samples, insert_earlier, name));
    warmup.shutdown();
    assert!(
        warmup.wait_termination_timeout(START_TIMEOUT),
        "warmup scheduler did not terminate"
    );
    drop(warmup_guard);

    let mut p50s = Vec::with_capacity(rounds);
    let mut p95s = Vec::with_capacity(rounds);
    for round_index in 0..rounds {
        let service = SingleThreadScheduledExecutorService::new("scheduled-timer").expect("scheduler should start");
        let guard = StopOnDrop(&service);
        let mut sorted = timer_round(&service, samples, insert_earlier, name);
        sorted.sort_unstable();
        let p50 = sorted[statistics::nearest_rank_index(sorted.len(), 50)];
        let p95 = sorted[statistics::nearest_rank_index(sorted.len(), 95)];
        println!(
            "sample,{name},{round_index},start_lateness_p50_ns={},start_lateness_p95_ns={},samples={samples},samples_unit=count",
            p50.as_nanos(),
            p95.as_nanos()
        );
        p50s.push(p50);
        p95s.push(p95);
        service.shutdown();
        assert!(
            service.wait_termination_timeout(START_TIMEOUT),
            "{name} scheduler did not terminate"
        );
        drop(guard);
    }
    p50s.sort_unstable();
    p95s.sort_unstable();
    println!(
        "summary,{name},median_start_lateness_p50_ns={},median_start_lateness_p95_ns={},samples={samples},samples_unit=count",
        statistics::median_ns(&p50s),
        statistics::median_ns(&p95s)
    );
}

fn timer_round(
    service: &SingleThreadScheduledExecutorService,
    samples: usize,
    insert_earlier: bool,
    workload: &str,
) -> Vec<Duration> {
    let mut lateness = Vec::with_capacity(samples);
    for sample_index in 0..samples {
        let sentinel = if insert_earlier {
            Some(
                service
                    .schedule_callable(SENTINEL_DELAY, || Ok::<(), ()>(()))
                    .expect("sentinel should be accepted"),
            )
        } else {
            None
        };
        let deadline = Instant::now() + TIMER_DELAY;
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let handle = service
            .schedule_callable_at(deadline, move || {
                let actual_start = Instant::now();
                let _ = started_tx.send(actual_start);
                Ok::<Instant, ()>(actual_start)
            })
            .expect("timer probe should be accepted");
        let actual_start = match started_rx.recv_timeout(START_TIMEOUT) {
            Ok(start) => start,
            Err(error) => {
                let _ = service.stop();
                panic!("{workload} sample {sample_index} did not start within {START_TIMEOUT:?}: {error}");
            }
        };
        let result_start = handle.get().expect("timer probe should succeed");
        assert_eq!(actual_start, result_start);
        assert!(
            actual_start >= deadline,
            "{workload} sample {sample_index} started before deadline"
        );
        lateness.push(actual_start.duration_since(deadline));
        if let Some(sentinel) = sentinel {
            assert_eq!(
                sentinel.cancel(),
                CancelResult::Cancelled,
                "sentinel should be cancelled"
            );
            assert!(matches!(
                sentinel.get(),
                Err(qubit_executor::TaskExecutionError::Cancelled)
            ));
        }
    }
    lateness
}

struct DropProbe(Arc<AtomicUsize>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn run_cancel_workload(rounds: usize) {
    let warmup =
        SingleThreadScheduledExecutorService::new("scheduled-cancel-warmup").expect("warmup scheduler should start");
    let warmup_guard = StopOnDrop(&warmup);
    black_box(cancel_round(&warmup));
    warmup.shutdown();
    assert!(
        warmup.wait_termination_timeout(START_TIMEOUT),
        "warmup scheduler did not terminate"
    );
    drop(warmup_guard);

    let mut total_times = Vec::with_capacity(rounds);
    let mut average_times = Vec::with_capacity(rounds);
    for round_index in 0..rounds {
        let service = SingleThreadScheduledExecutorService::new("scheduled-cancel").expect("scheduler should start");
        let guard = StopOnDrop(&service);
        let (total, average, peak, remaining) = cancel_round(&service);
        println!(
            "sample,cancel_far_future,{round_index},cancel_total_ns={},cancel_average_ns={},cancel_average_unit=ns,queued_peak={peak},queued_peak_unit=count,queued_after_cancel={remaining},queued_after_cancel_unit=count,dropped={CANCEL_TASKS},dropped_unit=count",
            total.as_nanos(),
            average.as_nanos()
        );
        total_times.push(total);
        average_times.push(average);
        service.shutdown();
        assert!(
            service.wait_termination_timeout(START_TIMEOUT),
            "cancel scheduler did not terminate"
        );
        drop(guard);
    }
    total_times.sort_unstable();
    average_times.sort_unstable();
    println!(
        "summary,cancel_far_future,median_cancel_total_ns={},median_cancel_average_ns={},cancel_average_unit=ns,queued_after_cancel=0,queued_after_cancel_unit=count,dropped={CANCEL_TASKS},dropped_unit=count",
        statistics::median_ns(&total_times),
        statistics::median_ns(&average_times)
    );
}

fn cancel_round(service: &SingleThreadScheduledExecutorService) -> (Duration, Duration, usize, usize) {
    let dropped = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(CANCEL_TASKS);
    for _ in 0..CANCEL_TASKS {
        let probe = DropProbe(Arc::clone(&dropped));
        handles.push(
            service
                .schedule_callable(SENTINEL_DELAY, move || {
                    let _probe = &probe;
                    Ok::<(), ()>(())
                })
                .expect("far-future task should be accepted"),
        );
    }
    let peak = service.queued_count();
    let started = Instant::now();
    for handle in &handles {
        assert_eq!(handle.cancel(), CancelResult::Cancelled);
    }
    let total = started.elapsed();
    let remaining = service.queued_count();
    assert_eq!(remaining, 0, "all cancelled tasks should leave the queue");
    for handle in handles {
        assert!(matches!(
            handle.get(),
            Err(qubit_executor::TaskExecutionError::Cancelled)
        ));
    }
    assert_eq!(
        dropped.load(Ordering::SeqCst),
        CANCEL_TASKS,
        "all captured probes should be dropped"
    );
    (total, total / CANCEL_TASKS as u32, peak, remaining)
}

fn sequential_round(service: &SingleThreadScheduledExecutorService) -> (Duration, Vec<Duration>) {
    let started = Instant::now();
    let mut latency = Vec::with_capacity(TASKS);
    for _ in 0..TASKS {
        let submitted = Instant::now();
        let handle = service
            .schedule_callable(Duration::ZERO, || Ok::<usize, ()>(42))
            .expect("task should be accepted");
        assert_eq!(black_box(handle.get()).expect("task should succeed"), 42);
        latency.push(submitted.elapsed());
    }
    (started.elapsed(), latency)
}

fn batch_round(service: &SingleThreadScheduledExecutorService) -> (Duration, Vec<Duration>) {
    let started = Instant::now();
    let mut submitted = Vec::with_capacity(TASKS);
    let mut handles = Vec::with_capacity(TASKS);
    for _ in 0..TASKS {
        submitted.push(Instant::now());
        handles.push(
            service
                .schedule_callable(Duration::ZERO, || Ok::<usize, ()>(42))
                .expect("task should be accepted"),
        );
    }
    let mut latency = Vec::with_capacity(TASKS);
    for (submitted, handle) in submitted.into_iter().zip(handles) {
        assert_eq!(black_box(handle.get()).expect("task should succeed"), 42);
        latency.push(submitted.elapsed());
    }
    (started.elapsed(), latency)
}
