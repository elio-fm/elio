use super::*;

#[test]
fn finalize_deduplicates_trackers_and_synthesizes_single_file_contents() {
    let mut metadata = TorrentMetadata {
        name: Some("elio-v1.0.0.tar.gz".to_string()),
        announce: Some("https://tracker.example/primary".to_string()),
        announce_tiers: vec![
            vec![
                "https://tracker.example/backup".to_string(),
                "https://tracker.example/primary".to_string(),
            ],
            vec![
                "".to_string(),
                "https://tracker.example/backup".to_string(),
                "https://tracker.example/third".to_string(),
            ],
        ],
        total_size: Some(4_096),
        ..TorrentMetadata::default()
    };

    metadata.finalize();

    assert_eq!(metadata.mode, Some(TorrentMode::SingleFile));
    assert_eq!(
        metadata.announce_tiers,
        vec![
            vec![
                "https://tracker.example/backup".to_string(),
                "https://tracker.example/primary".to_string(),
            ],
            vec!["https://tracker.example/third".to_string()],
        ]
    );
    assert_eq!(metadata.file_count, 1);
    assert_eq!(metadata.files.len(), 1);
    assert_eq!(metadata.files[0].path, "elio-v1.0.0.tar.gz");
    assert_eq!(metadata.files[0].length, 4_096);
    assert_eq!(
        metadata.tracker_fields(),
        vec![
            (
                "Tier 1".to_string(),
                "https://tracker.example/backup  •  https://tracker.example/primary".to_string(),
            ),
            (
                "Tier 2".to_string(),
                "https://tracker.example/third".to_string(),
            ),
        ]
    );
}

#[test]
fn multifile_content_entries_are_rooted_under_torrent_name() {
    let metadata = TorrentMetadata {
        name: Some("elio-release".to_string()),
        mode: Some(TorrentMode::MultiFile),
        file_count: 2,
        files: vec![
            TorrentFileEntry {
                path: "docs/readme.txt".to_string(),
                length: 12,
            },
            TorrentFileEntry {
                path: "bin/elio".to_string(),
                length: 34,
            },
        ],
        ..TorrentMetadata::default()
    };

    let entries = metadata.content_entries();

    assert_eq!(
        entries,
        vec![
            ArchiveEntry {
                path: "elio-release/docs/readme.txt".to_string(),
                is_dir: false,
            },
            ArchiveEntry {
                path: "elio-release/bin/elio".to_string(),
                is_dir: false,
            },
        ]
    );
}
