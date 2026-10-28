// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::any::Any;
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;

/// Discards a panic payload without allowing its destructor to unwind again.
///
/// If dropping the original payload panics, the secondary payload is
/// intentionally leaked because dropping it could trigger another panic. This
/// contains recoverable unwinding only; it cannot recover from process aborts.
pub(crate) fn discard_panic_payload(payload: Box<dyn Any + Send>) {
    if let Err(secondary) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
        std::mem::forget(secondary);
    }
}

#[cfg(test)]
mod tests {
    use std::panic;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use super::discard_panic_payload;

    static SECONDARY_DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

    struct PanicsWithSecondaryPayload;

    impl Drop for PanicsWithSecondaryPayload {
        fn drop(&mut self) {
            panic::panic_any(SecondaryPayload);
        }
    }

    struct SecondaryPayload;

    impl Drop for SecondaryPayload {
        fn drop(&mut self) {
            SECONDARY_DROP_COUNT.fetch_add(1, Ordering::Relaxed);
            panic!("secondary payload must be forgotten");
        }
    }

    #[test]
    fn test_discard_panic_payload_forgets_secondary_payload() {
        SECONDARY_DROP_COUNT.store(0, Ordering::Relaxed);
        let payload = panic::catch_unwind(|| panic::panic_any(PanicsWithSecondaryPayload))
            .expect_err("panic payload should be captured");

        discard_panic_payload(payload);

        assert_eq!(SECONDARY_DROP_COUNT.load(Ordering::Relaxed), 0);
    }
}
