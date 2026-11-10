// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::time::Duration;

/// Returns the median duration in nanoseconds from a non-empty sorted sample.
///
/// For an even number of samples, the middle two values are averaged and any
/// fractional nanosecond is rounded down.
pub fn median_ns(sorted: &[Duration]) -> u128 {
    assert!(!sorted.is_empty(), "median requires at least one sample");
    let middle = sorted.len() / 2;
    let hi = sorted[middle].as_nanos();
    if sorted.len() % 2 == 1 {
        return hi;
    }
    let lo = sorted[middle - 1].as_nanos();
    lo + (hi - lo) / 2
}

/// Returns the zero-based index for a nearest-rank percentile.
///
/// The sample count must be non-zero and the percentile must be from 1 to 100.
pub fn nearest_rank_index(sample_count: usize, percentile: usize) -> usize {
    assert!(sample_count > 0, "percentiles require at least one sample");
    assert!((1..=100).contains(&percentile), "percentile must be in 1..=100");
    ((sample_count as u128 * percentile as u128).div_ceil(100) - 1) as usize
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_median_uses_middle_value_for_odd_samples() {
        assert_eq!(
            super::median_ns(&[
                std::time::Duration::from_nanos(1),
                std::time::Duration::from_nanos(3),
                std::time::Duration::from_nanos(9)
            ]),
            3
        );
    }

    #[test]
    fn test_median_averages_middle_values_for_even_samples() {
        assert_eq!(
            super::median_ns(&[std::time::Duration::from_nanos(1), std::time::Duration::from_nanos(9)]),
            5
        );
        assert_eq!(
            super::median_ns(&[std::time::Duration::from_nanos(1), std::time::Duration::from_nanos(2)]),
            1
        );
    }

    #[test]
    fn test_median_handles_single_and_repeated_samples() {
        assert_eq!(super::median_ns(&[std::time::Duration::from_nanos(7)]), 7);
        assert_eq!(
            super::median_ns(&[std::time::Duration::from_nanos(7), std::time::Duration::from_nanos(7)]),
            7
        );
    }

    #[test]
    fn test_median_handles_maximum_duration_without_overflow() {
        assert_eq!(
            super::median_ns(&[std::time::Duration::MAX, std::time::Duration::MAX]),
            std::time::Duration::MAX.as_nanos()
        );
    }

    #[test]
    #[should_panic(expected = "at least one")]
    fn test_median_rejects_empty_input() {
        super::median_ns(&[]);
    }

    #[test]
    fn test_nearest_rank_uses_one_based_rank() {
        assert_eq!(super::nearest_rank_index(10_000, 50), 4_999);
        assert_eq!(super::nearest_rank_index(10_000, 95), 9_499);
        assert_eq!(super::nearest_rank_index(1, 1), 0);
        assert_eq!(super::nearest_rank_index(1, 100), 0);
        assert_eq!(super::nearest_rank_index(usize::MAX, 100), usize::MAX - 1);
    }

    #[test]
    #[should_panic(expected = "at least one")]
    fn test_nearest_rank_rejects_empty_samples() {
        super::nearest_rank_index(0, 50);
    }

    #[test]
    #[should_panic(expected = "1..=100")]
    fn test_nearest_rank_rejects_zero_percentile() {
        super::nearest_rank_index(1, 0);
    }

    #[test]
    #[should_panic(expected = "1..=100")]
    fn test_nearest_rank_rejects_percentile_over_one_hundred() {
        super::nearest_rank_index(1, 101);
    }
}
