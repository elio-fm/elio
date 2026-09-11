use super::*;

fn candidate(name: &str, size: u64) -> CandidateFile {
    CandidateFile {
        path: PathBuf::from(name),
        name: name.to_string(),
        relative: name.to_string(),
        size,
        modified: None,
        cache_key: None,
    }
}

#[test]
fn candidate_bucket_order_does_not_let_huge_low_count_files_starve_cheap_groups() {
    let huge_size = 16 * 1024 * 1024 * 1024;
    let mut buckets = [
        (0..6)
            .map(|index| candidate(&format!("huge-{index}"), huge_size))
            .collect::<Vec<_>>(),
        (0..20)
            .map(|index| candidate(&format!("small-{index}"), 1024))
            .collect::<Vec<_>>(),
    ];

    buckets.sort_by(|left, right| compare_candidate_buckets(left, right));

    assert_eq!(buckets[0][0].size, 1024);
    assert_eq!(buckets[1][0].size, huge_size);
}
