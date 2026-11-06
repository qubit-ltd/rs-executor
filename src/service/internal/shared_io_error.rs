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

/// Preserves the structured details of an I/O error behind shared ownership.
///
/// This wrapper allows multiple owners to retain one error allocation while
/// still exposing the original error as its source.
#[derive(Debug)]
pub struct SharedIoError {
    /// Shared allocation containing the original I/O error.
    source: Arc<io::Error>,
}

impl SharedIoError {
    /// Wraps a shared I/O error without discarding its structured context.
    ///
    /// # Parameters
    ///
    /// * `source` - Shared original I/O error.
    pub fn new(source: Arc<io::Error>) -> Self {
        Self { source }
    }
}

impl fmt::Display for SharedIoError {
    /// Formats the wrapped I/O error using its original display implementation.
    ///
    /// # Parameters
    ///
    /// * `self` - Shared I/O error wrapper to format.
    /// * `f` - Formatter receiving the error text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.source.as_ref(), f)
    }
}

impl Error for SharedIoError {
    /// Returns the wrapped I/O error as the source error.
    ///
    /// # Parameters
    ///
    /// * `self` - Shared I/O error wrapper whose source is requested.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}
