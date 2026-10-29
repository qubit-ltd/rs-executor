# Code Formatting Guide

## Recommended workflow

Run the repository's alignment and CI checks from the repository root:

```bash
./.infra/bin/align-ci.sh
git diff --check
./.infra/bin/ci-check.sh
```

`align-ci.sh` applies the formatting and style fixes managed by the rs-infra
style tool. It writes changes to the working tree, so review the resulting diff
before keeping it. `git diff --check` checks for whitespace errors, and
`ci-check.sh` runs the repository CI checks.

## Formatting configuration

Rust formatting is configured in [`.infra/style/rustfmt.toml`](.infra/style/rustfmt.toml).
The project uses Rust 2024 formatting, a 120-column maximum width, and
`imports_granularity = "Item"` with `group_imports = "StdExternalCrate"`.

`Item` formats imports at the item level. `StdExternalCrate` groups imports by
standard library, external crate, and the current crate, with blank lines
between groups. For example:

```rust
use std::io;

use serde::Serialize;

use crate::task::Task;
```

## Toolchain responsibilities

The project build toolchain is stable Rust 1.94.0, as specified by
[`rust-toolchain.toml`](rust-toolchain.toml) and
[`.infra/tools/defaults.toml`](.infra/tools/defaults.toml). The infrastructure
defaults pin `nightly-2026-06-05` for its Clippy and nightly tool roles. These
settings describe tool-specific responsibilities; they do not mean that every
CI job runs on nightly.

## CI

The GitHub Actions workflow is [`.github/workflows/ci.yml`](.github/workflows/ci.yml).
It delegates to the shared rs-infra CI workflow. Use `./.infra/bin/ci-check.sh`
to run the repository's CI checks locally; use `./.infra/bin/align-ci.sh` to
apply style fixes before reviewing the diff.

## Troubleshooting

If alignment changes files, inspect `git diff` to confirm the changes are
expected. If a CI check reports a formatting issue, run the recommended
alignment command and review the diff, then rerun the CI check. Check that the
repository's configured toolchains and rs-infra settings match the files above
when a tool reports a version mismatch.
