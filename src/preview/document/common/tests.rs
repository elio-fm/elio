use super::*;

fn contains_iso_datetime_separator(value: &str) -> bool {
    value.as_bytes().windows(16).any(|window| {
        window[0..4].iter().all(u8::is_ascii_digit)
            && window[4] == b'-'
            && window[5..7].iter().all(u8::is_ascii_digit)
            && window[7] == b'-'
            && window[8..10].iter().all(u8::is_ascii_digit)
            && window[10] == b'T'
            && window[11..13].iter().all(u8::is_ascii_digit)
            && window[13] == b':'
            && window[14..16].iter().all(u8::is_ascii_digit)
    })
}

// --- parse_offset_minutes ---

#[test]
fn parse_offset_minutes_z_is_zero() {
    assert_eq!(parse_offset_minutes("Z"), Some(0));
}

#[test]
fn parse_offset_minutes_positive_offsets() {
    assert_eq!(parse_offset_minutes("+00:00"), Some(0));
    assert_eq!(parse_offset_minutes("+01:00"), Some(60));
    assert_eq!(parse_offset_minutes("+05:30"), Some(330));
    assert_eq!(parse_offset_minutes("+14:00"), Some(840));
}

#[test]
fn parse_offset_minutes_negative_offsets() {
    assert_eq!(parse_offset_minutes("-05:00"), Some(-300));
    assert_eq!(parse_offset_minutes("-11:30"), Some(-690));
}

#[test]
fn parse_offset_minutes_rejects_non_numeric() {
    assert_eq!(parse_offset_minutes("UTC"), None);
    assert_eq!(parse_offset_minutes("CET"), None);
    assert_eq!(parse_offset_minutes(""), None);
}

#[test]
fn parse_offset_minutes_rejects_malformed() {
    // Non-numeric minutes field must not silently become 0.
    assert_eq!(parse_offset_minutes("+05:xx"), None);
    assert_eq!(parse_offset_minutes("-08:??"), None);
    // Extra segments must be rejected.
    assert_eq!(parse_offset_minutes("+05:30:99"), None);
    // Out-of-range hours.
    assert_eq!(parse_offset_minutes("+15:00"), None);
    assert_eq!(parse_offset_minutes("-15:00"), None);
    // Missing minutes field (hours-only form is not used in document metadata).
    assert_eq!(parse_offset_minutes("+05"), None);
    // Out-of-range minutes.
    assert_eq!(parse_offset_minutes("+05:60"), None);
    assert_eq!(parse_offset_minutes("+05:99"), None);
}

// --- format_offset_label ---

#[test]
fn format_offset_label_zero_is_utc() {
    assert_eq!(format_offset_label(0), "UTC");
}

#[test]
fn format_offset_label_positive_and_negative() {
    assert_eq!(format_offset_label(60), "+01:00");
    assert_eq!(format_offset_label(330), "+05:30");
    assert_eq!(format_offset_label(-300), "-05:00");
    assert_eq!(format_offset_label(-690), "-11:30");
}

// --- days_from_civil / to_unix_seconds ---

#[test]
fn days_from_civil_unix_epoch() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
}

#[test]
fn to_unix_seconds_utc_at_known_epoch() {
    // Jan 1 1970 00:00 UTC = unix second 0
    assert_eq!(to_unix_seconds(1970, 1, 1, 0, 0, 0), 0);
}

#[test]
fn to_unix_seconds_strips_source_offset() {
    // 09:00 +02:00 is the same instant as 07:00 Z
    let via_plus2 = to_unix_seconds(2026, 3, 11, 9, 0, 120);
    let via_utc = to_unix_seconds(2026, 3, 11, 7, 0, 0);
    assert_eq!(via_plus2, via_utc);
}

// --- humanize_document_datetime ---

#[test]
fn humanize_datetime_never_returns_raw_iso_with_tz() {
    // Any ISO 8601 string with an explicit offset must be transformed.
    let cases = [
        "2026-03-11T09:00:00Z",
        "2026-03-11T09:00:00+00:00",
        "2026-03-11T09:00:00+05:30",
        "2026-03-11T09:00:00-08:00",
    ];
    for input in &cases {
        let result = humanize_document_datetime(input);
        assert_ne!(result, *input, "raw ISO not transformed: {input}");
        assert!(
            !contains_iso_datetime_separator(&result),
            "raw ISO datetime still present in: {result}"
        );
        assert!(
            result.contains("Mar"),
            "month abbreviation missing: {result}"
        );
        assert!(result.contains("2026"), "year missing: {result}");
    }
}

#[test]
fn humanize_datetime_without_tz_shows_no_label() {
    // When the source carries no timezone information, show it as-is
    // without inventing one.
    let result = humanize_document_datetime("2026-03-11T09:00:00");
    assert_eq!(result, "Mar 11, 2026 09:00");
}

#[test]
fn humanize_datetime_invalid_passes_through() {
    assert_eq!(humanize_document_datetime("not a date"), "not a date");
    // Missing separator between date and time
    assert_eq!(humanize_document_datetime("20260311"), "20260311");
}

/// Equivalent instants expressed in different source offsets must produce
/// identical output after local-time conversion, regardless of the machine
/// timezone. This is the strongest timezone-independent correctness check:
/// it fails if the UTC normalisation step is wrong.
#[test]
fn humanize_datetime_equivalent_instants_give_same_output() {
    let utc = humanize_document_datetime("2026-03-11T07:00:00Z");
    let plus2 = humanize_document_datetime("2026-03-11T09:00:00+02:00");
    let minus5 = humanize_document_datetime("2026-03-11T02:00:00-05:00");
    assert_eq!(utc, plus2, "+02:00 not correctly normalised to UTC");
    assert_eq!(utc, minus5, "-05:00 not correctly normalised to UTC");
}

/// When the system timezone is UTC (typical in CI), verify the full
/// output string exactly. Skipped silently on non-UTC machines so the
/// suite stays green everywhere.
#[test]
fn humanize_datetime_exact_output_on_utc_systems() {
    let local_offset = unix_to_local(0)
        .map(|(_, _, _, _, _, off)| off)
        .unwrap_or(1);
    if local_offset != 0 {
        return; // not a UTC machine — skip
    }
    assert_eq!(
        humanize_document_datetime("2026-03-11T09:00:00Z"),
        "Mar 11, 2026 09:00 UTC",
    );
    assert_eq!(
        humanize_document_datetime("2026-03-11T09:00:00+02:00"),
        "Mar 11, 2026 07:00 UTC",
    );
}

// --- humanize_pdfinfo_datetime / try_humanize_ctime_datetime ---

#[test]
fn pdfinfo_datetime_reformats_ctime_without_timezone() {
    // No TZ suffix: pdfinfo already emits local time; reformat only.
    assert_eq!(
        humanize_pdfinfo_datetime("Wed Mar 11 09:00:00 2026"),
        "Mar 11, 2026 09:00",
    );
}

#[test]
fn pdfinfo_datetime_preserves_named_timezone_label() {
    // Named TZ (ambiguous): reformat and keep the original label.
    assert_eq!(
        humanize_pdfinfo_datetime("Wed Mar 11 10:00:00 2026 CET"),
        "Mar 11, 2026 10:00 CET",
    );
}

#[test]
fn pdfinfo_datetime_ctime_utc_matches_iso_utc() {
    // These represent the same instant; after local conversion they must be identical.
    let ctime = humanize_pdfinfo_datetime("Wed Mar 11 07:00:00 2026 UTC");
    let iso = humanize_pdfinfo_datetime("2026-03-11T07:00:00Z");
    assert_eq!(
        ctime, iso,
        "ctime UTC and ISO UTC should give the same local-time output"
    );
}

#[test]
fn pdfinfo_datetime_still_handles_iso() {
    // ISO 8601 (poppler ≥ 22.02 without offset) → reformatted, no label.
    assert_eq!(
        humanize_pdfinfo_datetime("2026-03-11 09:00:00"),
        "Mar 11, 2026 09:00",
    );
}

#[test]
fn pdfinfo_datetime_exact_utc_output_on_utc_systems() {
    let local_offset = unix_to_local(0)
        .map(|(_, _, _, _, _, off)| off)
        .unwrap_or(1);
    if local_offset != 0 {
        return;
    }
    assert_eq!(
        humanize_pdfinfo_datetime("Wed Mar 11 09:00:00 2026 UTC"),
        "Mar 11, 2026 09:00 UTC",
    );
}

#[test]
fn pdfinfo_datetime_passes_through_unrecognised() {
    assert_eq!(humanize_pdfinfo_datetime("not a date"), "not a date");
    // Five tokens but month field is not a recognised abbreviation.
    assert_eq!(
        humanize_pdfinfo_datetime("Wed Xxx 11 09:00:00 2026"),
        "Wed Xxx 11 09:00:00 2026",
    );
}
