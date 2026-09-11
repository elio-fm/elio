use super::*;

#[test]
fn torrent_preview_shows_single_file_metadata_and_trackers() {
    let root = temp_path("torrent");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("sample.torrent");
    let bytes = bencode_dict(vec![
        ("announce", bencode_str("https://tracker.test")),
        (
            "announce-list",
            bencode_list(vec![bencode_list(vec![
                bencode_str("https://tracker.test"),
                bencode_str("https://backup.test"),
            ])]),
        ),
        ("comment", bencode_str("test torrent")),
        ("created by", bencode_str("elio")),
        (
            "info",
            bencode_dict(vec![
                ("length", bencode_int(12_345)),
                ("name", bencode_str("file.txt")),
                ("piece length", bencode_int(262_144)),
                ("pieces", bencode_bytes(b"12345678901234567890")),
                ("private", bencode_int(1)),
            ]),
        ),
    ]);
    fs::write(&path, bytes).expect("failed to write torrent");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Text);
    assert_eq!(preview.detail.as_deref(), Some("BitTorrent file"));
    assert_eq!(line_texts.first().map(String::as_str), Some("Details"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Name") && text.contains("file.txt"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Mode") && text.contains("Single-file"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Private")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Trackers") && text.contains("2 across 1 tier"))
    );
    assert!(line_texts.iter().any(|text| text == "Trackers"));
    assert!(line_texts.iter().any(|text| {
        text.contains("Tier 1") && text.contains("tracker.test") && text.contains("backup.test")
    }));
    assert!(line_texts.iter().any(|text| text == "Contents"));
    assert!(line_texts.iter().any(|text| text.contains("file.txt")));
    assert!(!preview.truncated);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn torrent_preview_shows_multifile_contents_tree() {
    let root = temp_path("torrent-multifile");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("series.torrent");
    let bytes = bencode_dict(vec![
        (
            "announce-list",
            bencode_list(vec![
                bencode_list(vec![
                    bencode_str("https://tracker.one"),
                    bencode_str("https://tracker.two"),
                ]),
                bencode_list(vec![bencode_str("https://backup.tld/announce")]),
            ]),
        ),
        ("created by", bencode_str("elio")),
        (
            "info",
            bencode_dict(vec![
                (
                    "files",
                    bencode_list(vec![
                        bencode_dict(vec![
                            ("length", bencode_int(100)),
                            (
                                "path",
                                bencode_list(vec![
                                    bencode_str("season-01"),
                                    bencode_str("ep1.mkv"),
                                ]),
                            ),
                        ]),
                        bencode_dict(vec![
                            ("length", bencode_int(200)),
                            (
                                "path.utf-8",
                                bencode_list(vec![
                                    bencode_str("season-01"),
                                    bencode_str("ep2.mkv"),
                                ]),
                            ),
                        ]),
                    ]),
                ),
                ("name", bencode_str("series")),
                ("piece length", bencode_int(65_536)),
                (
                    "pieces",
                    bencode_bytes(b"1234567890123456789012345678901234567890"),
                ),
                ("private", bencode_int(0)),
            ]),
        ),
    ]);
    fs::write(&path, bytes).expect("failed to write torrent");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Text);
    assert_eq!(preview.detail.as_deref(), Some("BitTorrent file"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Mode") && text.contains("Multi-file"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Files") && text.contains("2"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Trackers") && text.contains("3 across 2 tiers"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Tier 2")));
    assert!(line_texts.iter().any(|text| text.contains("series/")));
    assert!(line_texts.iter().any(|text| text.contains("season-01/")));
    assert!(line_texts.iter().any(|text| text.contains("ep1.mkv")));
    assert!(line_texts.iter().any(|text| text.contains("ep2.mkv")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Privacy") && text.contains("Public"))
    );
    assert!(!preview.truncated);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
