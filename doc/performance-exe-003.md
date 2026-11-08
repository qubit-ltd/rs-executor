# EXE-003 ScheduledExecutor measurement report

## Decision

**Benefit not established; keep the production worker unchanged.** The candidate did not meet the required primary-workload gates: sequential median total time improved only 2.76% and only 11/15 pairs favored it (required >=5% and >=12/15). The sequential per-task p50 also improved only 1.96%. Batch results were inconsistent (9/15 total-time wins). Measurements show substantial run-to-run noise; no runtime allocation count was measured.

The candidate correctness check passed all 12 worker-focused unit tests. The production change was then reverted exactly to the worktree baseline; only this report, benchmark, and Cargo bench registration remain as T3 changes.

## Method and environment

- Host: `mas-server`, Linux `6.8.0-147-generic`, x86_64; Intel Xeon E5-2680 v4 @ 2.40 GHz, 56 logical CPUs, 2 sockets, 28 cores. `lscpu` reported scaling range 1200–3300 MHz.
- Toolchain: `rustc 1.94.0 (4a4ef493e 2026-03-02)`, LLVM 21.1.8; `cargo 1.94.0 (85eff7c80 2026-01-15)`. `rust-toolchain.toml` pins 1.94.0.
- Release bench profile: Cargo default optimized bench profile (`opt-level=3`); no custom `RUSTFLAGS` supplied.
- Lockfile-resolved key dependencies: `qubit-function 0.18.3`, `qubit-lock 0.14.4`, `qubit-collections 0.2.3`, `parking_lot 0.12.5`, `oneshot 0.2.1` (from Cargo build output / lockfile).
- Workloads: zero-delay callable returning `Ok::<usize, ()>(42)`. Sequential submits then gets each result; batch submits 10,000 handles then gets all results. Service construction and shutdown/termination wait are outside each timed interval. Per-task latency is submission-to-result observation; sequential mode therefore includes the blocking wait and is not pure queue time.
- Each regular invocation performs one warmup and 15 rounds of 10,000 tasks per workload. For paired trials, the same benchmark was run one measured round at a time, baseline then candidate, alternating for 15 pairs; every invocation retained one full warmup. Values are nanoseconds. The corrected benchmark computes nearest-rank p50/p95 from 10,000 per-task elapsed samples using index `ceil(percentile * n) - 1` (indices 4999 and 9499 for n=10,000).
- The parent coordinator reported that a separate existing worktree on this same machine was running a heavy Rust coverage/build job during the measurement window. This creates a known shared-host load confounder; it aligns with the large pair-to-pair variation and strengthens the conservative no-benefit conclusion. No independent CPU pinning or frequency isolation was used.
- Source SHA-256: baseline worker `d3fc1499bce8d8c1576979deeb87ffb92da3b3300046087d3c6ff76ec902e406`; candidate worker `eee51439268cc9554e9f25c11793e16b0c810f858b20fe74c743d6a12e81cf1a`; benchmark `4093d91aea5bc681321ac6e5e2fd0f4b7754a8eeed71e1c457dc72dbf94f8bd8`.
- Benchmark source records static wrapping path only. No profiler or allocator counter was used, so no runtime allocation count is claimed.

## Paired raw results

Each metric cell is `total / task p50 / task p95`, in nanoseconds. **The p50/p95 values below are from the original paired runs, whose index was one rank too high** (`5000`/`9500`, rather than nearest-rank `4999`/`9499`). Raw per-task samples were not retained, so these percentile values cannot be recalculated. The total-time measurements and the optimization gate are unaffected; percentile figures are approximate historical observations and are not presented as corrected nearest-rank values. As requested, the paired gate was not rerun for this statistics-only correction.

| Pair | Seq baseline total/p50/p95 (ns) | Seq candidate total/p50/p95 (ns) | Batch baseline total/p50/p95 (ns) | Batch candidate total/p50/p95 (ns) |
|---:|---:|---:|---:|---:|
| 1 | 130607295/12886/14071 | 114626730/11285/11932 | 20709190/10636193/17639612 | 17863031/8918393/14947161 |
| 2 | 105468781/10401/11165 | 112409499/11005/11813 | 15157179/7662988/12490596 | 17382050/8645335/14576148 |
| 3 | 106127790/10259/11970 | 106159550/10262/11787 | 16575027/8689040/13765648 | 15982193/8110468/13476526 |
| 4 | 122446586/12092/12902 | 119991487/11855/13395 | 18572154/9090336/15529617 | 14689949/7843741/11725641 |
| 5 | 185722266/18465/20972 | 98756852/9737/10197 | 34839243/17515642/30537196 | 14519411/7589848/11832152 |
| 6 | 124245094/12659/14050 | 113826334/10984/12432 | 18745147/9697147/15483200 | 19274565/9255512/16551114 |
| 7 | 114598470/11295/12052 | 114102733/11247/11811 | 17732354/9159492/14886705 | 17935151/8666346/15170997 |
| 8 | 108019528/10643/11193 | 112244544/11046/11669 | 15249127/7374870/12396328 | 17235756/8475348/14404929 |
| 9 | 108252945/10385/11951 | 100890788/10035/10409 | 18434555/9741382/15832839 | 13885782/7199463/10911769 |
| 10 | 112068805/10025/14389 | 116416363/11398/12407 | 25925800/14572344/21675569 | 18243520/8877938/15052940 |
| 11 | 111315158/11035/12448 | 98076708/9750/10401 | 16249566/8199231/13816030 | 14203932/7723541/11514676 |
| 12 | 116344231/11100/14828 | 113065111/11083/11912 | 15651137/7635433/13167475 | 18367205/8876172/15730612 |
| 13 | 119790781/11760/12768 | 101224712/9831/11602 | 18479448/8828410/15605315 | 16613099/7610195/13824827 |
| 14 | 117261329/11444/12562 | 114030572/11045/14846 | 18440665/9306698/15527478 | 17514920/8437469/14715026 |
| 15 | 115961025/10511/15808 | 112898952/10924/12070 | 15530058/7878687/12441429 | 16583542/8319499/13815332 |

## Paired summary

| Metric | Baseline median | Candidate median | Median pair improvement | Candidate wins |
|---|---:|---:|---:|---:|
| Sequential total | 115961025 | 112898952 | 2.76% | 11/15 |
| Sequential task p50 | 11100 | 11005 | 1.96% | 10/15 |
| Sequential task p95 | 12768 | 11813 | 11.52% | 11/15 |
| Batch total | 18434555 | 17235756 | 5.02% | 9/15 |
| Batch task p50 | 9090336 | 8437469 | 6.66% | 11/15 |
| Batch task p95 | 15483200 | 14404929 | 5.23% | 9/15 |

Improvement is `(baseline - candidate) / baseline`; a positive value favors the candidate. The required primary gate is sequential total time. Batch is a secondary workload and p95 guard. There were multiple >5% losses as well as wins, so results are noisy and the retention criteria are not satisfied.

## Percentile correction follow-up

The benchmark now selects the nearest-rank element with `(sample_count * percentile).div_ceil(100) - 1`. A post-correction smoke run on the unchanged baseline worker succeeded; its 15-round summaries were sequential total/p50/p95 `143755493 / 14206 / 14857 ns` and batch `14483664 / 7204102 / 11689362 ns`. These are a smoke run under the same shared-host load and are not part of the paired candidate comparison.

## Commands and outcomes

- `cargo bench --bench scheduled_executor_bench` (initial baseline, after fixing a benchmark compile-time assertion): exit 0.
- `cargo test --lib schedule::scheduled_worker::tests` (candidate worker): exit 0, 12 passed, 0 failed.
- `cargo bench --bench scheduled_executor_bench` (candidate, 15 rounds): exit 0.
- Paired alternating baseline/candidate run: 15 pairs, each `SCHEDULED_BENCH_ROUNDS=1 cargo bench --quiet --bench scheduled_executor_bench`; all 30 invocations exited 0. Raw stdout was collected at `/tmp/scheduled-executor-paired-raw.txt` during this run; the complete paired measurements are included above.
- `rustfmt --edition 2024 benches/scheduled_executor_bench.rs` (initial and percentile correction): exit 0.
- `cargo bench --bench scheduled_executor_bench` after percentile correction, unchanged baseline worker: exit 0; one smoke invocation only, no optimization-gate remeasurement.
- `cargo fmt --all -- --check`: exit 1 because it reports formatting diffs throughout files outside T3's write set. No out-of-scope files were formatted or modified; the T3 benchmark itself was formatted directly.
- `cargo bench --bench scheduled_executor_bench` after restoring the baseline worker: exit 0. This final run showed further elevated sequential timings, consistent with environmental noise; it is not used as a substitute for the paired trials.

The first bench compile attempt exited 101 because `TaskExecutionError<()>` does not implement `PartialEq`; the benchmark was corrected to unwrap and compare the successful value, then all subsequent release runs succeeded.
