"""Regression tests for the Markdown Rust example checker."""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


CHECKER_PATH = Path(__file__).with_name("check_markdown_examples.py")
SPEC = importlib.util.spec_from_file_location("check_markdown_examples", CHECKER_PATH)
assert SPEC is not None and SPEC.loader is not None
checker = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = checker
SPEC.loader.exec_module(checker)


class CheckMarkdownExamplesTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_root = tempfile.TemporaryDirectory(prefix="markdown-check-test-")
        self.addCleanup(self.temporary_root.cleanup)
        self.root = Path(self.temporary_root.name)
        (self.root / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
        for document in checker.DEFAULT_DOCUMENTS:
            path = self.root / document
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("# Fixture\n\n```rust\nfn main() {}\n```\n", encoding="utf-8")

    def run_checker(self, run: mock.Mock) -> int:
        with mock.patch.object(sys, "argv", [str(CHECKER_PATH), "--project-root", str(self.root)]):
            with mock.patch.object(checker.subprocess, "run", run):
                return checker.main()

    def test_target_cache_survives_fixture_cleanup_and_is_reused(self) -> None:
        target_dir = self.root / "target" / "markdown-example-check" / "cargo-target"
        fixture_dirs: list[Path] = []

        def compile_fixture(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
            manifest_index = command.index("--manifest-path") + 1
            fixture = Path(command[manifest_index]).parent
            fixture_dirs.append(fixture)
            environment = kwargs["env"]
            assert isinstance(environment, dict)
            self.assertEqual(Path(environment["CARGO_TARGET_DIR"]), target_dir)
            self.assertEqual(command[:3], ["cargo", "check", "--offline"])
            sentinel = target_dir / "cache-sentinel"
            if len(fixture_dirs) == 2:
                self.assertTrue(sentinel.is_file(), "cache should survive the first fixture cleanup")
            sentinel.parent.mkdir(parents=True, exist_ok=True)
            sentinel.write_text("reused", encoding="utf-8")
            return subprocess.CompletedProcess(command, 0, "", "")

        run = mock.Mock(side_effect=compile_fixture)
        for _ in range(2):
            self.assertEqual(self.run_checker(run), 0)

        self.assertEqual(run.call_count, 2)
        self.assertEqual(len(fixture_dirs), 2)
        for fixture in fixture_dirs:
            self.assertFalse(fixture.exists())
            self.assertNotIn(fixture, target_dir.parents)
            self.assertNotIn(target_dir, fixture.parents)
        self.assertTrue((target_dir / "cache-sentinel").is_file())

    def test_cargo_failure_is_propagated_with_source_document_location(self) -> None:
        def fail_with_source(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            manifest_index = command.index("--manifest-path") + 1
            fixture = Path(command[manifest_index]).parent
            generated = fixture / "src" / "bin" / "example_0001.rs"
            diagnostic = f"error: failed to compile {generated}:1\n"
            return subprocess.CompletedProcess(command, 17, "", diagnostic)

        run = mock.Mock(side_effect=fail_with_source)
        stderr = tempfile.TemporaryFile(mode="w+")
        try:
            with mock.patch.object(sys, "stderr", stderr):
                result = self.run_checker(run)
            stderr.seek(0)
            output = stderr.read()
        finally:
            stderr.close()

        self.assertEqual(result, 17)
        self.assertIn("Source: " + str(self.root / "README.md") + ":3 (Rust fence)", output)


if __name__ == "__main__":
    unittest.main()
