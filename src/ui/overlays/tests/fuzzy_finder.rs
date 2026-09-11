use super::*;

#[test]
fn limited_search_summary_keeps_limit_status_when_width_is_tight() {
    let summary = build_search_summary(false, true, 127_053, 5_000_000, 45);

    assert_eq!(summary, "scan limit reached  •  5,000,000 scanned");
}

#[test]
fn loading_search_summary_prioritizes_scanning_status_when_width_is_tight() {
    let summary = build_search_summary(true, false, 127_053, 1_000_000, 40);

    assert_eq!(summary, "scanning…  •  1,000,000 scanned");
}

#[test]
fn loading_search_summary_keeps_count_order_when_width_allows() {
    let summary = build_search_summary(true, false, 86_472, 518_144, 60);

    assert_eq!(summary, "86,472 results  •  518,144 scanned  •  scanning…");
}
