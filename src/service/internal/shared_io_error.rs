// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::error::Error;
use std::fmt;
use std::io;
use std::sync::Arc;

/// Preserves an I/O error while another owner retains its shared allocation.
#[derive(Debug)]
pub struct SharedIoError {
    source: Arc<io::Error>,
}

impl SharedIoError {
    /// Wraps a shared I/O error without discarding its structured context.
    pub fn new(source: Arc<io::Error>) -> Self {
        Self { source }
    }
}

impl fmt::Display for SharedIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.source.as_ref(), f)
    }
}

impl Error for SharedIoError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}
