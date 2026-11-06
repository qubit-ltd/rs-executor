// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Runs selected integration-test bodies in a child process with a deadline.

use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::process::Child;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

const CHILD_MARKER_ENV: &str = "QUBIT_EXECUTOR_TEST_WATCHDOG_CHILD";
const LOG_PREFIX: &str = "QUBIT_EXECUTOR_TEST_WATCHDOG_ENTERED:";
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
static NEXT_LOG_ID: AtomicU64 = AtomicU64::new(0);

/// Runs `body` in a supervised copy of the current integration-test binary.
pub(crate) fn run_test(test_name: &str, marker: &str, timeout: Duration, body: impl FnOnce()) -> Result<(), String> {
    if env::var(CHILD_MARKER_ENV).as_deref() == Ok(marker) {
        println!("{LOG_PREFIX}{marker}");
        io::stdout()
            .flush()
            .map_err(|error| format!("flush child handshake: {error}"))?;
        body();
        return Ok(());
    }

    supervise(test_name, marker, timeout)
}

fn supervise(test_name: &str, marker: &str, timeout: Duration) -> Result<(), String> {
    let log = TempLog::create().map_err(|error| format!("create watchdog log: {error}"))?;
    let child = Command::new(env::current_exe().map_err(|error| format!("locate test binary: {error}"))?)
        .arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(CHILD_MARKER_ENV, marker)
        .stdout(Stdio::from(
            log.file
                .as_ref()
                .expect("watchdog log handle is open")
                .try_clone()
                .map_err(|error| format!("clone watchdog log: {error}"))?,
        ))
        .stderr(Stdio::from(
            log.file
                .as_ref()
                .expect("watchdog log handle is open")
                .try_clone()
                .map_err(|error| format!("clone watchdog log: {error}"))?,
        ))
        .spawn()
        .map_err(|error| format!("start child for {test_name}: {error}"))?;
    let mut child_guard = ChildGuard::new(child);
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| format!("watchdog deadline is not representable for {test_name}"))?;

    loop {
        match child_guard.child.as_mut().expect("child guard owns child").try_wait() {
            Ok(Some(status)) => {
                child_guard.disarm();
                let output = log
                    .read_to_string()
                    .map_err(|error| format!("read watchdog log: {error}"))?;
                return validate_exit(test_name, marker, status, &output);
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                let child = child_guard.child.as_mut().expect("child guard owns child");
                let _ = child.kill();
                let status = child
                    .wait()
                    .map_err(|error| format!("reap timed-out child for {test_name}: {error}"))?;
                child_guard.disarm();
                let output = log
                    .read_to_string()
                    .map_err(|error| format!("read watchdog log: {error}"))?;
                return Err(format!(
                    "test child {test_name} exceeded {timeout:?} and was terminated (status {status}); output:\n{output}"
                ));
            }
            Err(error) => return Err(format!("poll child for {test_name}: {error}")),
        }
    }
}

fn validate_exit(test_name: &str, marker: &str, status: ExitStatus, output: &str) -> Result<(), String> {
    if !status.success() {
        return Err(format!(
            "test child {test_name} exited with {status}; output:\n{output}"
        ));
    }
    if !output.contains(&format!("{LOG_PREFIX}{marker}")) {
        return Err(format!(
            "test child {test_name} exited without entering its body; output:\n{output}"
        ));
    }
    Ok(())
}

struct ChildGuard {
    child: Option<Child>,
    armed: bool,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self {
            child: Some(child),
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.armed
            && let Some(child) = self.child.as_mut()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct TempLog {
    path: std::path::PathBuf,
    file: Option<File>,
}

impl TempLog {
    fn create() -> io::Result<Self> {
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        loop {
            let id = NEXT_LOG_ID.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "executor-test-watchdog-{}-{since_epoch}-{id}.log",
                std::process::id()
            ));
            match File::options().write(true).read(true).create_new(true).open(&path) {
                Ok(file) => {
                    return Ok(Self { path, file: Some(file) });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    fn read_to_string(&self) -> io::Result<String> {
        let mut file = File::open(&self.path)?;
        let mut output = String::new();
        file.read_to_string(&mut output)?;
        Ok(output)
    }
}

impl Drop for TempLog {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}
