# Executor adapter contracts

This independent Cargo test fixture runs shared executor-service contract assertions against the Rayon and Tokio adapters and both thread pool implementations. It requires four sibling checkouts:

```text
rust-common/
├── rs-executor/
├── rs-rayon-executor/
├── rs-thread-pool/
└── rs-tokio-executor/
```

Local runs use the current working trees in those sibling directories. CI checks out adapter revisions pinned in `.github/workflows/ci.yml`. The fixture lockfile pins registry dependencies; sibling path dependencies resolve to the checked-out source trees and do not pin their Git revisions.

From the `rs-executor` repository root, verify the resolved source paths and run the fixture:

```bash
cargo metadata --locked --format-version 1 --manifest-path tests/adapter_contracts/Cargo.toml > /tmp/executor-adapter-metadata.json
python3 -B tests/internal/check_adapter_sources.py --metadata /tmp/executor-adapter-metadata.json
cargo test --locked --manifest-path tests/adapter_contracts/Cargo.toml
```

The fixture contains a byte-for-byte copy of `tests/internal/process_watchdog.rs`. The Rust style standard requires test modules to use normal module paths, so the fixture cannot redirect to the root file with `#[path]` or `include!`. A Python unittest compares the two files byte-for-byte to prevent drift; run it with:

```bash
python3 -B -m unittest discover -s tests/internal -p check_adapter_sources_tests.py
```
The fixture checks successful results, business failures, shutdown rejection, cancellation of work known to be pending, Running task cancellation, and that dropping either an unpolled or polled result future does not cancel queued work. The Running cancellation case waits for an explicit started handshake, checks `AlreadyRunning`, releases the callable, verifies its original value, then checks `AlreadyFinished`. It checks concurrent cancellation against pending work across Rayon, Tokio, and the fixed and dynamic thread pools: two cancellation calls race, one must cancel, one must report that the task is already finished, and the cancelled result must arrive. These checks verify the observable contract and task termination; they do not prove every possible thread interleaving. Shared coverage runs against Rayon, Tokio blocking, and the fixed and dynamic thread pools. Its local path patch ensures all providers use this checkout of `qubit-executor`.

Each provider also has a `stop_contract` case. A one-worker blocker keeps one accepted task running while a tracked task is queued. Stop must reject later submissions, cancel the pending task without invoking its callable, and leave termination unobserved with a zero timeout until the blocker is released. After release, the blocker result must arrive and termination must be observed. The tests intentionally do not compare `StopReport` fields: the providers document different counting rules. Rayon reports running, queued, and cancelled work; Tokio reports a snapshot plus tracked blocking jobs aborted before their closures start; fixed and dynamic pools use count-based reports, with the dynamic pool counting only jobs removed and cancelled by that stop call. Tokio coverage uses `TokioExecutorService` and blocking closures, not the separate asynchronous IO service.

The fixed and dynamic thread-pool fixtures also cover a capacity-one cancellation lifecycle. Standard tracked cancellation publishes a cancelled result, but does not actively remove the pool job from the queue; the worker later skips the cancelled callable and drops its captures. The fixture checks that the queue remains full until that cleanup, that capacity is then available, and that a subsequent blocker plus queued task still observes the one-item capacity limit. These provider-specific assertions do not extend the generic `ExecutorService` cancellation contract. The thread-pool `PoolJobTicket` API has a separate queue-removal mechanism and is outside this fixture's tracked-cancellation behavior.

The Rayon and Tokio repositories retain their focused provider-specific tests. This fixture adds shared cross-adapter coverage and does not replace those suites.
