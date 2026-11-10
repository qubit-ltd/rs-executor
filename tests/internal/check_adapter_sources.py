#!/usr/bin/env python3
"""Check that adapter contract metadata resolves the intended local checkouts."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

TARGET_PACKAGES = (
    "qubit-executor",
    "qubit-rayon-executor",
    "qubit-tokio-executor",
    "qubit-thread-pool",
)


def expected_manifests(script_path: Path = Path(__file__)) -> dict[str, Path]:
    """Return the expected package manifests in this sibling checkout layout."""
    executor_root = script_path.resolve().parents[2]
    sibling_root = executor_root.parent
    return {
        "qubit-executor": executor_root / "Cargo.toml",
        "qubit-rayon-executor": sibling_root / "rs-rayon-executor" / "Cargo.toml",
        "qubit-tokio-executor": sibling_root / "rs-tokio-executor" / "Cargo.toml",
        "qubit-thread-pool": sibling_root / "rs-thread-pool" / "Cargo.toml",
    }


def check_metadata(metadata: Any, expected: dict[str, Path]) -> list[str]:
    """Return all source and path errors found in Cargo metadata."""
    if not isinstance(metadata, dict) or not isinstance(metadata.get("packages"), list):
        return ["metadata must contain a packages array"]

    errors: list[str] = []
    packages = metadata["packages"]
    for name in TARGET_PACKAGES:
        matches = [package for package in packages if isinstance(package, dict) and package.get("name") == name]
        if len(matches) != 1:
            errors.append(f"{name}: expected exactly one package, found {len(matches)}")
            continue

        package = matches[0]
        source = package.get("source", "<missing>")
        manifest = package.get("manifest_path")
        if source is not None:
            errors.append(f"{name}: expected local source (null), found {source!r}; manifest={manifest!r}")
        if not isinstance(manifest, str):
            errors.append(f"{name}: missing string manifest_path; found {manifest!r}")
            continue

        actual_path = Path(manifest).resolve()
        expected_path = expected[name].resolve()
        if actual_path != expected_path:
            errors.append(f"{name}: expected manifest {expected_path}, found {actual_path}")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--metadata", type=Path, required=True, help="cargo metadata JSON file")
    args = parser.parse_args()

    try:
        with args.metadata.open(encoding="utf-8") as metadata_file:
            metadata = json.load(metadata_file)
    except (OSError, json.JSONDecodeError) as error:
        print(f"cannot read Cargo metadata from {args.metadata}: {error}")
        return 1

    errors = check_metadata(metadata, expected_manifests())
    if errors:
        print("adapter dependency source check failed:")
        for error in errors:
            print(f"- {error}")
        return 1

    for name, manifest in expected_manifests().items():
        print(f"{name}: {manifest.resolve()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
