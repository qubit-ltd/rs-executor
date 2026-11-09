// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::path::PathBuf;
use std::process::Command;

#[test]
fn test_markdown_checker_unit_tests() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("python3")
        .args([
            "-B",
            "-m",
            "unittest",
            "discover",
            "-s",
            "tests/internal",
            "-p",
            "check_markdown_examples_tests.py",
        ])
        .current_dir(&root)
        .output()
        .expect("Python 3 must be available to run Markdown checker unit tests");

    assert!(
        output.status.success(),
        "Markdown checker unit tests failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn test_markdown_rust_examples_compile() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("python3")
        .arg(root.join("tests/internal/check_markdown_examples.py"))
        .arg("--project-root")
        .arg(&root)
        .output()
        .expect("Python 3 must be available to compile Markdown examples");

    assert!(
        output.status.success(),
        "Markdown Rust examples failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Ran 6 opt-in Rust examples."),
        "Markdown checker did not run all six opted-in examples:\n{stdout}"
    );
}
