#!/usr/bin/env python3
"""Compile Rust fences from the project's four user-facing Markdown files."""

from __future__ import annotations

import argparse
import json
import shutil
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

DEFAULT_DOCUMENTS = (
    "README.md",
    "README.zh_CN.md",
    "doc/user_guide.md",
    "doc/user_guide.zh_CN.md",
)
SUPPORTED_TAGS = {"no_run", "ignore"}
RUST_VERSION_TAG = re.compile(r"^edition(2015|2018|2021|2024)$")


def extract_examples(document: Path) -> list[tuple[int, str, str]]:
    lines = document.read_text(encoding="utf-8").splitlines(keepends=True)
    examples: list[tuple[int, str, str]] = []
    fence_char: str | None = None
    fence_size = 0
    code: list[str] = []
    start_line = 0
    ignored = False
    edition = "2024"

    for line_number, line in enumerate(lines, 1):
        marker = re.match(r"^\s*(`{3,}|~{3,})(.*)$", line.rstrip("\r\n"))
        if marker is None:
            if fence_char is not None:
                code.append(line)
            continue

        fence = marker.group(1)
        info = marker.group(2).strip()
        if fence_char is None:
            fence_char = fence[0]
            fence_size = len(fence)
            if fence_char == "`" and "`" in info:
                raise ValueError(f"{document}:{line_number}: backticks are not allowed in fence info")
            tokens = info.replace(",", " ").split()
            is_rust = bool(tokens and tokens[0].lower() == "rust")
            if is_rust:
                unknown = [tag for tag in tokens[1:] if tag not in SUPPORTED_TAGS and not RUST_VERSION_TAG.match(tag)]
                if unknown:
                    raise ValueError(f"{document}:{line_number}: unsupported Rust fence tag(s): {', '.join(unknown)}")
                ignored = "ignore" in tokens[1:]
                edition_tag = next((tag for tag in tokens[1:] if RUST_VERSION_TAG.match(tag)), "edition2024")
                edition = edition_tag.removeprefix("edition")
                start_line = line_number
                code = []
            else:
                start_line = 0
                code = []
        elif fence[0] == fence_char and len(fence) >= fence_size and not info:
            if start_line:
                if not ignored:
                    examples.append((start_line, "".join(code), edition))
                start_line = 0
                code = []
            fence_char = None
            fence_size = 0

    if fence_char is not None:
        raise ValueError(f"{document}: unclosed Markdown fence")
    return examples


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--project-root", type=Path, required=True)
    parser.add_argument("--documents", nargs="+", default=DEFAULT_DOCUMENTS)
    args = parser.parse_args()
    root = args.project_root.resolve()
    documents = [(root / name).resolve() for name in args.documents]
    missing = [path for path in documents if not path.is_file()]
    if missing:
        print("Missing Markdown document(s): " + ", ".join(map(str, missing)), file=sys.stderr)
        return 2

    examples: list[tuple[Path, int, str, str]] = []
    try:
        for document in documents:
            examples.extend((document, line, source, edition) for line, source, edition in extract_examples(document))
    except (OSError, UnicodeError, ValueError) as error:
        print(error, file=sys.stderr)
        return 2
    if not examples:
        print("No Rust examples found in the requested Markdown documents.", file=sys.stderr)
        return 2

    editions = {edition for _, _, _, edition in examples}
    if len(editions) != 1:
        print("Rust examples use mixed editions; split them into separate fixture packages.", file=sys.stderr)
        return 2
    edition = next(iter(editions))

    build_root = root / "target" / "markdown-example-check"
    build_root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="fixture-", dir=build_root) as temporary:
        fixture = Path(temporary)
        source_dir = fixture / "src" / "bin"
        source_dir.mkdir(parents=True)
        manifest = fixture / "Cargo.toml"
        manifest.write_text(
            "[package]\nname = \"markdown-example-check\"\nversion = \"0.0.0\"\nedition = " + json.dumps(edition) + "\n\n"
            "[dependencies]\nqubit-executor = { path = " + json.dumps(str(root)) + " }\n",
            encoding="utf-8",
        )
        shutil.copyfile(root / "Cargo.lock", fixture / "Cargo.lock")
        origins: dict[str, tuple[Path, int]] = {}
        for index, (document, line, source, edition) in enumerate(examples, 1):
            generated = source_dir / f"example_{index:04}.rs"
            generated.write_text(source, encoding="utf-8")
            origins[generated.name] = (document, line)
            display_path = document.relative_to(root) if document.is_relative_to(root) else document
            print(f"Checking {display_path} Rust block {index} (fence line {line}, edition {edition})")

        target_dir = fixture / "cargo-target"
        environment = os.environ.copy()
        environment["CARGO_TARGET_DIR"] = str(target_dir)
        command = ["cargo", "check", "--offline", "--bins", "--manifest-path", str(manifest)]
        result = subprocess.run(command, cwd=root, env=environment, text=True, capture_output=True, check=False)
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr:
            print(result.stderr, end="", file=sys.stderr)
        if result.returncode:
            print("Markdown example compilation failed.", file=sys.stderr)
            for generated in re.findall(r"(?:^|\s)([^\s:]+/example_\d{4}\.rs):\d+", result.stdout + result.stderr, re.MULTILINE):
                source_name = Path(generated).name
                if source_name in origins:
                    document, line = origins[source_name]
                    print(f"Source: {document}:{line} (Rust fence)", file=sys.stderr)
            return result.returncode
        print(f"Compiled {len(examples)} Rust examples from {len(documents)} Markdown documents.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
