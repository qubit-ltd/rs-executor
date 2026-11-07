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

The fixture checks successful results, business failures, shutdown rejection, cancellation of work known to be pending, and that dropping either an unpolled or polled result future does not cancel queued work. Shared coverage runs against Rayon, Tokio, and the fixed and dynamic thread pools. Its local path patch ensures all providers use this checkout of `qubit-executor`.

The Rayon and Tokio repositories retain their focused provider-specific tests. This fixture adds shared cross-adapter coverage and does not replace those suites.
