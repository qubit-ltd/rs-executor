"""Tests for adapter dependency source metadata validation."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import check_adapter_sources as checker


class CheckAdapterSourcesTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary_directory.cleanup)
        base = Path(self.temporary_directory.name)
        self.expected = {
            name: base / name / "Cargo.toml" for name in checker.TARGET_PACKAGES
        }
        self.metadata = {
            "packages": [
                {
                    "name": name,
                    "source": None,
                    "manifest_path": str(path),
                }
                for name, path in self.expected.items()
            ]
        }

    def test_expected_local_sources_pass(self) -> None:
        self.assertEqual(checker.check_metadata(self.metadata, self.expected), [])

    def test_registry_source_fails(self) -> None:
        self.metadata["packages"][0]["source"] = "registry+https://github.com/rust-lang/crates.io-index"
        self.assertTrue(checker.check_metadata(self.metadata, self.expected))

    def test_other_checkout_fails(self) -> None:
        self.metadata["packages"][0]["manifest_path"] = str(
            Path(self.temporary_directory.name) / "other" / "Cargo.toml"
        )
        self.assertTrue(checker.check_metadata(self.metadata, self.expected))

    def test_duplicate_executor_fails(self) -> None:
        self.metadata["packages"].append(self.metadata["packages"][0].copy())
        self.assertTrue(checker.check_metadata(self.metadata, self.expected))

    def test_missing_package_fails(self) -> None:
        self.metadata["packages"] = [
            package for package in self.metadata["packages"] if package["name"] != "qubit-thread-pool"
        ]
        self.assertTrue(checker.check_metadata(self.metadata, self.expected))

    def test_missing_manifest_field_fails(self) -> None:
        del self.metadata["packages"][0]["manifest_path"]
        self.assertTrue(checker.check_metadata(self.metadata, self.expected))

    def test_invalid_metadata_shape_fails(self) -> None:
        self.assertTrue(checker.check_metadata({}, self.expected))

    def test_fixture_watchdog_copy_matches_root_source(self) -> None:
        repository_root = Path(__file__).resolve().parents[2]
        root_source = repository_root / "tests/internal/process_watchdog.rs"
        fixture_source = repository_root / "tests/adapter_contracts/tests/process_watchdog.rs"
        self.assertEqual(fixture_source.read_bytes(), root_source.read_bytes())

    def test_command_line_reports_failure(self) -> None:
        script = Path(checker.__file__).resolve()
        with tempfile.TemporaryDirectory() as directory:
            metadata_file = Path(directory) / "metadata.json"
            metadata_file.write_text(json.dumps({"packages": []}), encoding="utf-8")
            result = subprocess.run(
                [sys.executable, "-B", str(script), "--metadata", str(metadata_file)],
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("qubit-executor", result.stdout)


if __name__ == "__main__":
    unittest.main()
