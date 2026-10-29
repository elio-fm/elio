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

#[test]
fn recycle_bin_candidates_hide_sidecars_and_use_original_names() {
    let bin = std::env::temp_dir().join(format!(
        "elio-duplicate-recycle-bin-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&bin).expect("failed to create bin");
    crate::filesystem::write_test_pair(&bin, "0A5E05.txt", r"C:\x\alpha.txt", 1_710_498_600);
    crate::filesystem::write_test_pair(&bin, "0B6F16.txt", r"C:\x\beta.txt", 1_710_498_600);
    let recycled_dir =
        crate::filesystem::write_test_pair(&bin, "0C7D27", r"C:\x\Projects", 1_710_498_600);
    fs::remove_file(&recycled_dir).expect("replace recycled file with directory");
    fs::create_dir_all(&recycled_dir).expect("failed to create recycled directory");
    fs::write(recycled_dir.join("notes.txt"), "recycled content")
        .expect("failed to write nested file");
    fs::write(bin.join("desktop.ini"), "system").expect("failed to write desktop.ini");
    let _bin = crate::filesystem::override_recycle_bin_for_test(&bin);

    let (groups, _) = collect_size_candidates(&bin, true, &|| false)
        .expect("failed to collect recycle bin candidates");
    let names = groups
        .values()
        .flatten()
        .map(|file| file.name.as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"alpha.txt"));
    assert!(names.contains(&"beta.txt"));
    assert!(
        groups
            .values()
            .flatten()
            .any(|file| file.relative == "Projects/notes.txt")
    );
    assert!(
        !names
            .iter()
            .any(|name| name.starts_with("$I") || name.starts_with("$R"))
    );
    assert!(!names.contains(&"desktop.ini"));

    fs::remove_dir_all(bin).expect("failed to remove bin");
}
