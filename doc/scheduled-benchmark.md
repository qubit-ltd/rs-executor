# Scheduled executor benchmark

`scheduled_executor_bench` is a lightweight, dependency-free benchmark executable. It complements the historical paired experiment in [performance-exe-003.md](performance-exe-003.md); its results do not replace or revise that report. Each workload gets one warmup round, excluded from reported samples. A fresh scheduler service is used for each measured round, and service construction and normal shutdown are outside each workload's timed interval.

## Workloads and metrics

| Workload | Work per measured round | Reported metrics |
| --- | --- | --- |
| `sequential` | Submit and observe 10,000 zero-delay tasks one at a time. | Total round time (`ns`), submission-to-result-observation task latency p50/p95 (`ns`). |
| `batch` | Submit 10,000 zero-delay tasks, then observe their results. | Total round time (`ns`), submission-to-result-observation latency p50/p95 (`ns`). This is not task start or completion latency. |
| `future_deadline` | Submit 64 independent probes serially, each with a deadline 5 ms after submission. | Closure start lateness p50/p95 (`ns`), measured from the scheduled deadline to `Instant::now()` inside the task closure. |
| `earlier_deadline` | For each of 64 probes, enqueue a 60 s sentinel, then submit a probe due in 5 ms. Cancel the sentinel after the probe completes. | Closure start lateness p50/p95 (`ns`), measured inside the probe closure. The sentinel must cancel successfully and report `Cancelled`. |
| `cancel_far_future` | Enqueue 1,000 tasks with 60 s deadlines, then cancel all of them. Queue construction is outside the cancellation timer. | Cancellation phase total and per-task mean (`ns`); peak and post-cancel queued count (`count`); captured-value drop count (`count`). |

For timer workloads, a bounded start notification detects probes that fail to start within five seconds. This is a failure timeout, not a performance threshold. Every timer sample verifies that the closure started at or after its deadline. The earlier-deadline workload measures insertion of a nearer task while a later task is queued. The public API cannot prove that the worker was already blocked on the sentinel, so this workload does not claim deterministic coverage of the internal wait-interruption interleaving; scheduler unit tests cover that race.

Nearest-rank p50/p95 indices are computed independently within each round. Cross-round summaries use the median of the per-round values. For an even number of rounds, the median is the arithmetic mean of the two middle nanosecond values, rounded down to an integer nanosecond. The original `sequential` and `batch` sample field layout is retained. Their summary behavior changes only for even round counts, which now average the middle values instead of selecting the upper middle value.

`queued_count` and captured-value drop counts are logical resource-lifecycle indicators. They are not measurements of allocation count, resident memory, or thread joining. The benchmark does not install a global allocator or sample process memory.

## Configuration and reproduction

Defaults are 15 measured rounds, one warmup per workload, 64 timer samples per timer round, 10,000 immediate tasks per existing workload, 1,000 far-future cancellation tasks, 5 ms probe deadlines, and 60 s sentinel deadlines. Configure rounds and timer samples with positive integers:

```bash
SCHEDULED_BENCH_ROUNDS=15 SCHEDULED_BENCH_TIMER_SAMPLES=64 \
  timeout 600s cargo bench --bench scheduled_executor_bench
```

`SCHEDULED_BENCH_TIMER_SAMPLES=0` and non-integer values fail with an explicit diagnostic. Benchmark output is line-oriented CSV-like text. Fields named `*_ns` are nanoseconds; fields named `*_unit=count` are counts. Keep stdout and stderr from each run together with the source revision, lockfile digest, OS, CPU, Rust toolchain, workload parameters, and any competing system load. Compare baseline and candidate with identical settings, alternating runs on the same machine. Do not interpret machine-dependent timing noise as a general pass/fail threshold.

## Baseline

The implementation-stage baseline is recorded in [scheduled-benchmark-baseline-20261110.txt](scheduled-benchmark-baseline-20261110.txt). It is a baseline for this source revision only; no candidate comparison or improvement claim is made.
