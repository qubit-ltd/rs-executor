# Executor adapter contracts

This independent Cargo test fixture runs shared executor-service contract assertions against the Rayon and Tokio adapters and both thread pool implementations. It uses the sibling checkout layout expected by its path dependencies:

```text
rust-common/
├── rs-executor/
├── rs-rayon-executor/
├── rs-thread-pool/
└── rs-tokio-executor/
```

From the `rs-executor` repository root, run:

```bash
cargo test --manifest-path tests/adapter_contracts/Cargo.toml
```

The fixture checks successful results, business failures, shutdown rejection, cancellation of work known to be pending, and that dropping either an unpolled or polled result future does not cancel queued work. It checks concurrent cancellation against pending work across Rayon, Tokio, and the fixed and dynamic thread pools: two cancellation calls race, one must cancel, one must report that the task is already finished, and the cancelled result must arrive. These checks verify the observable contract and task termination; they do not prove every possible thread interleaving. Shared coverage runs against Rayon, Tokio, and the fixed and dynamic thread pools. Its local path patch ensures all providers use this checkout of `qubit-executor`.

The fixed and dynamic thread-pool fixtures also cover a capacity-one cancellation lifecycle. Standard tracked cancellation publishes a cancelled result, but does not actively remove the pool job from the queue; the worker later skips the cancelled callable and drops its captures. The fixture checks that the queue remains full until that cleanup, that capacity is then available, and that a subsequent blocker plus queued task still observes the one-item capacity limit. These provider-specific assertions do not extend the generic `ExecutorService` cancellation contract. The thread-pool `PoolJobTicket` API has a separate queue-removal mechanism and is outside this fixture's tracked-cancellation behavior.

The Rayon and Tokio repositories retain their focused provider-specific tests. This fixture adds shared cross-adapter coverage and does not replace those suites.
