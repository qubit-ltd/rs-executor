"""Regression tests for the Markdown Rust example checker."""

from __future__ import annotations

import importlib.util
import json
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

    def test_parser_marks_only_opted_in_examples_runnable(self) -> None:
        document = self.root / "README.md"
        document.write_text(
            "```rust\nfn ordinary() {}\n```\n"
            "```rust,executor_run\nfn runnable() {}\n```\n"
            "```rust,ignore\nfn ignored() {}\n```\n"
            "```rust,no_run\nfn compile_only() {}\n```\n",
            encoding="utf-8",
        )

        examples = checker.extract_examples(document)

        self.assertEqual([example[0] for example in examples], [1, 4, 10])
        self.assertEqual([example[3] for example in examples], [False, True, False])

    def test_parser_rejects_executor_run_with_ignore_or_no_run(self) -> None:
        for tags in ("executor_run,ignore", "no_run,executor_run"):
            with self.subTest(tags=tags):
                document = self.root / "README.md"
                document.write_text(f"```rust,{tags}\nfn main() {{}}\n```\n", encoding="utf-8")
                with self.assertRaisesRegex(ValueError, rf"{document}:1:.*executor_run"):
                    checker.extract_examples(document)

    def test_parser_still_rejects_unknown_tags_and_unclosed_fences(self) -> None:
        document = self.root / "README.md"
        for source, message in (
            ("```rust,unknown\nfn main() {}\n```\n", "unsupported Rust fence tag"),
            ("```rust\nfn main() {}\n", "unclosed Markdown fence"),
        ):
            with self.subTest(message=message):
                document.write_text(source, encoding="utf-8")
                with self.assertRaisesRegex(ValueError, message):
                    checker.extract_examples(document)

    def test_parser_preserves_supported_edition_tags(self) -> None:
        document = self.root / "README.md"
        document.write_text(
            "```rust,edition2018\nfn first() {}\n```\n"
            "```rust,edition2021\nfn second() {}\n```\n",
            encoding="utf-8",
        )
        self.assertEqual([example[2] for example in checker.extract_examples(document)], ["2018", "2021"])

    def test_unmarked_examples_only_run_cargo_check(self) -> None:
        run = mock.Mock(return_value=subprocess.CompletedProcess([], 0, "", ""))
        self.assertEqual(self.run_checker(run), 0)
        self.assertEqual(run.call_count, 1)
        self.assertEqual(run.call_args.args[0][:3], ["cargo", "check", "--offline"])

    def test_runnable_examples_are_built_and_executed_after_check(self) -> None:
        document = self.root / "README.md"
        document.write_text("```rust,executor_run\nfn main() {}\n```\n", encoding="utf-8")
        executable = self.root / "example_0001"
        artifact = json.dumps(
            {"reason": "compiler-artifact", "target": {"name": "example_0001"}, "executable": str(executable)}
        )
        calls: list[list[str]] = []
        fixture_dirs: list[Path] = []
        target_dirs: list[Path] = []

        def run_process(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
            calls.append(command)
            if "--manifest-path" in command:
                fixture_dirs.append(Path(command[command.index("--manifest-path") + 1]).parent)
                environment = kwargs["env"]
                assert isinstance(environment, dict)
                target_dir = Path(environment["CARGO_TARGET_DIR"])
                target_dirs.append(target_dir)
                target_dir.mkdir(parents=True, exist_ok=True)
                (target_dir / "sentinel").write_text("cached", encoding="utf-8")
            if command[:2] == ["cargo", "check"]:
                return subprocess.CompletedProcess(command, 0, "", "")
            if command[:2] == ["cargo", "build"]:
                return subprocess.CompletedProcess(command, 0, artifact + "\n", "")
            return subprocess.CompletedProcess(command, 0, "program output", "")

        run = mock.Mock(side_effect=run_process)
        stdout = tempfile.TemporaryFile(mode="w+")
        try:
            with mock.patch.object(sys, "stdout", stdout):
                self.assertEqual(self.run_checker(run), 0)
            stdout.seek(0)
            output = stdout.read()
        finally:
            stdout.close()

        self.assertEqual(run.call_count, 3)
        check_command, build_command, execute_command = calls
        self.assertEqual(check_command[:3], ["cargo", "check", "--offline"])
        self.assertEqual(build_command[:3], ["cargo", "build", "--offline"])
        self.assertIn("--bin", build_command)
        self.assertEqual(execute_command, [str(executable)])
        self.assertEqual(run.call_args_list[-1].kwargs["timeout"], 15)
        self.assertIn("Ran 1", output)
        self.assertIn("program output", output)
        self.assertEqual(build_command[-2:], ["--bin", "example_0001"])
        self.assertEqual(len(fixture_dirs), 2)
        self.assertTrue(all(not fixture.exists() for fixture in fixture_dirs))
        self.assertEqual(target_dirs[0], target_dirs[1])
        self.assertTrue((target_dirs[0] / "sentinel").is_file())

    def test_build_failure_reports_source_and_preserves_diagnostics(self) -> None:
        document = self.root / "README.md"
        document.write_text("```rust,executor_run\nfn main() {}\n```\n", encoding="utf-8")
        run = mock.Mock(
            side_effect=[
                subprocess.CompletedProcess([], 0, "", ""),
                subprocess.CompletedProcess([], 17, "cargo output", "build diagnostic"),
            ]
        )
        stderr = tempfile.TemporaryFile(mode="w+")
        try:
            with mock.patch.object(sys, "stderr", stderr):
                result = self.run_checker(run)
            stderr.seek(0)
            output = stderr.read()
        finally:
            stderr.close()
        self.assertEqual(result, 17)
        self.assertEqual(run.call_count, 2)
        self.assertIn(str(document) + ":1", output)
        self.assertIn("build diagnostic", output)

    def test_missing_executable_artifact_fails_with_source(self) -> None:
        document = self.root / "README.md"
        document.write_text("```rust,executor_run\nfn main() {}\n```\n", encoding="utf-8")
        run = mock.Mock(
            side_effect=[
                subprocess.CompletedProcess([], 0, "", ""),
                subprocess.CompletedProcess([], 0, "{}\n", ""),
            ]
        )
        stderr = tempfile.TemporaryFile(mode="w+")
        try:
            with mock.patch.object(sys, "stderr", stderr):
                result = self.run_checker(run)
            stderr.seek(0)
            output = stderr.read()
        finally:
            stderr.close()
        self.assertNotEqual(result, 0)
        self.assertIn(str(document) + ":1", output)
        self.assertIn("executable", output)

    def test_nonzero_and_timed_out_programs_fail_with_source(self) -> None:
        document = self.root / "README.md"
        document.write_text("```rust,executor_run\nfn main() {}\n```\n", encoding="utf-8")
        executable = self.root / "example_0001"
        artifact = json.dumps(
            {"reason": "compiler-artifact", "target": {"name": "example_0001"}, "executable": str(executable)}
        )
        cases = (
            (subprocess.CompletedProcess([], 9, "program stdout", "program stderr"), "exit status 9"),
            (subprocess.TimeoutExpired([str(executable)], 15, output=b"partial", stderr=b"timeout stderr"), "timed out"),
        )
        for program_result, expected in cases:
            with self.subTest(expected=expected):
                run = mock.Mock(
                    side_effect=[subprocess.CompletedProcess([], 0, "", ""), subprocess.CompletedProcess([], 0, artifact, ""), program_result]
                )
                stderr = tempfile.TemporaryFile(mode="w+")
                try:
                    with mock.patch.object(sys, "stderr", stderr):
                        result = self.run_checker(run)
                    stderr.seek(0)
                    output = stderr.read()
                finally:
                    stderr.close()
                self.assertNotEqual(result, 0)
                self.assertIn(str(document) + ":1", output)
                self.assertIn(expected, output)
                self.assertIn("example_0001", output)

    def test_unexpected_subprocess_error_reports_source(self) -> None:
        document = self.root / "README.md"
        document.write_text("```rust,executor_run\nfn main() {}\n```\n", encoding="utf-8")
        executable = self.root / "example_0001"
        artifact = json.dumps(
            {"reason": "compiler-artifact", "target": {"name": "example_0001"}, "executable": str(executable)}
        )
        run = mock.Mock(
            side_effect=[
                subprocess.CompletedProcess([], 0, "", ""),
                subprocess.CompletedProcess([], 0, artifact, ""),
                OSError("cannot start example"),
            ]
        )
        stderr = tempfile.TemporaryFile(mode="w+")
        try:
            with mock.patch.object(sys, "stderr", stderr):
                result = self.run_checker(run)
            stderr.seek(0)
            output = stderr.read()
        finally:
            stderr.close()
        self.assertNotEqual(result, 0)
        self.assertIn(str(document) + ":1", output)
        self.assertIn("cannot start example", output)

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
