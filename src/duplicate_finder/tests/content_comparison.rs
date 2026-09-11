use super::*;
use crate::duplicate_finder::scan_duplicates_streaming_with_cache;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "elio-duplicates-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn same_session_cache_reuses_unchanged_content_hashes() {
    let root = temp_path("hash-cache-hit");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.txt"), b"same").unwrap();
    fs::write(root.join("b.txt"), b"same").unwrap();
    let mut cache = DuplicateHashCache::default();

    let first =
        scan_duplicates_streaming_with_cache(&root, true, &mut cache, || false, |_| true).unwrap();
    let second =
        scan_duplicates_streaming_with_cache(&root, true, &mut cache, || false, |_| true).unwrap();

    assert_eq!(first.groups, second.groups);
    assert_eq!(first.stats.cached_hashes, 0);
    assert_eq!(first.stats.checked_candidates, first.stats.candidate_files);
    assert_eq!(second.stats.cached_hashes, second.stats.hashed_files);
    assert_eq!(
        second.stats.checked_candidates,
        second.stats.candidate_files
    );
    assert_eq!(second.stats.processed_bytes, 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn same_session_cache_misses_when_file_metadata_changes() {
    let root = temp_path("hash-cache-stale");
    fs::create_dir_all(&root).unwrap();
    let changed = root.join("a.txt");
    fs::write(&changed, b"same").unwrap();
    fs::write(root.join("b.txt"), b"same").unwrap();
    let mut cache = DuplicateHashCache::default();

    let first =
        scan_duplicates_streaming_with_cache(&root, true, &mut cache, || false, |_| true).unwrap();
    std::thread::sleep(Duration::from_millis(10));
    fs::write(&changed, b"diff").unwrap();
    let second =
        scan_duplicates_streaming_with_cache(&root, true, &mut cache, || false, |_| true).unwrap();

    assert_eq!(first.groups.len(), 1);
    assert!(second.groups.is_empty());
    assert_eq!(
        second.stats.checked_candidates,
        second.stats.candidate_files
    );
    assert!(second.stats.cached_hashes < second.stats.hashed_files);
    assert!(second.stats.processed_bytes > 0);
    fs::remove_dir_all(root).unwrap();
}
