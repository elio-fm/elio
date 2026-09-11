use super::{
    MIN_PREVIEW_WIDTH, MIN_RESULTS_WIDTH_WITH_PREVIEW, duplicate_group_label,
    duplicate_loading_status, duplicate_partial_status, duplicate_preview_width,
};
use crate::fs::duplicates::{DuplicateScanPhase, DuplicateScanStats};

#[test]
fn duplicate_group_labels_zero_pad_to_rank_width() {
    assert_eq!(duplicate_group_label(9, 1), "G9");
    assert_eq!(duplicate_group_label(9, 2), "G09");
    assert_eq!(duplicate_group_label(10, 2), "G10");
    assert_eq!(duplicate_group_label(100, 3), "G100");
}

#[test]
fn duplicate_preview_width_scales_with_available_space() {
    let compact = MIN_RESULTS_WIDTH_WITH_PREVIEW + MIN_PREVIEW_WIDTH;
    assert_eq!(duplicate_preview_width(compact), MIN_PREVIEW_WIDTH);
    assert_eq!(duplicate_preview_width(80), 20);
    assert_eq!(duplicate_preview_width(120), 34);
    assert_eq!(duplicate_preview_width(200), 60);
}

#[test]
fn duplicate_preview_width_is_monotonic_while_shrinking() {
    let first = MIN_RESULTS_WIDTH_WITH_PREVIEW + MIN_PREVIEW_WIDTH;
    let mut previous = duplicate_preview_width(first);
    for width in first + 1..220 {
        let current = duplicate_preview_width(width);
        assert!(
            current >= previous,
            "preview width regressed at total width {width}: {current} < {previous}"
        );
        previous = current;
    }
}

#[test]
fn duplicate_loading_status_shows_bytes_read_while_checking() {
    let status = duplicate_loading_status(
        DuplicateScanStats {
            phase: DuplicateScanPhase::ContentChecking,
            checked_candidates: 300_549,
            candidate_files: 300_552,
            processed_bytes: 170_000_000_000,
            cached_hashes: 42,
            duplicate_bytes: 21_000_000_000,
            ..DuplicateScanStats::default()
        },
        18,
    );

    assert_eq!(
        status,
        "checking… • 300549/300552 checked • 170 GB read • 42 cached • 18 groups • 21 GB reclaimable"
    );
}

#[test]
fn duplicate_partial_status_omits_bytes_read_after_stop() {
    let status = duplicate_partial_status(
        DuplicateScanStats {
            checked_candidates: 300_549,
            candidate_files: 300_552,
            processed_bytes: 170_000_000_000,
            duplicate_bytes: 21_000_000_000,
            ..DuplicateScanStats::default()
        },
        18,
    );

    assert_eq!(
        status,
        "partial results • 300549/300552 checked • 18 groups • 21 GB reclaimable"
    );
}
