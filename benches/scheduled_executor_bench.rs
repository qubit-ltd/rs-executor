// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::hint::black_box;
use std::time::Duration;
use std::time::Instant;

use qubit_executor::ExecutorService;
use qubit_executor::ScheduledExecutorService;
use qubit_executor::SingleThreadScheduledExecutorService;

const TASKS: usize = 10_000;

fn main() {
    let rounds = std::env::var("SCHEDULED_BENCH_ROUNDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(15);
    assert!(rounds > 0);
    println!("scheduled_executor_bench,tasks={TASKS},rounds={rounds},warmup=1");
    run_workload("sequential", rounds, sequential_round);
    run_workload("batch", rounds, batch_round);
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
        let p50 = latency[nearest_rank_index(latency.len(), 50)];
        let p95 = latency[nearest_rank_index(latency.len(), 95)];
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
        totals[rounds / 2].as_nanos(),
        p50s[rounds / 2].as_nanos(),
        p95s[rounds / 2].as_nanos()
    );
}

fn nearest_rank_index(sample_count: usize, percentile: usize) -> usize {
    assert!(sample_count > 0, "percentiles require at least one sample");
    assert!((1..=100).contains(&percentile), "percentile must be in 1..=100");
    (sample_count * percentile).div_ceil(100) - 1
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
