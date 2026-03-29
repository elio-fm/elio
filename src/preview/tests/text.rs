use super::*;

#[test]
fn plain_text_license_preview_shows_specific_license_detail() {
    let root = temp_path("plain-license");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("LICENSE");
    fs::write(
        &path,
        "MIT License\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the \"Software\"), to deal\nin the Software without restriction, including without limitation the rights\nto use, copy, modify, merge, publish, distribute, sublicense, and/or sell\ncopies of the Software, and to permit persons to whom the Software is\nfurnished to do so.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND.\n",
    )
    .expect("failed to write license");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Text);
    assert_eq!(preview.detail.as_deref(), Some("MIT License"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn srt_preview_keeps_specific_type_detail() {
    let root = temp_path("srt");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("movie.srt");
    fs::write(&path, "1\n00:00:01,000 --> 00:00:02,000\nHello\n").expect("failed to write srt");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Text);
    assert_eq!(preview.detail.as_deref(), Some("SubRip subtitles"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn text_preview_stays_plain() {
    let root = temp_path("text");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("notes.txt");
    fs::write(&path, "hello\nworld\n").expect("failed to write text");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Text);
    assert_eq!(preview.lines[0].spans.len(), 1);
    assert_eq!(preview.lines[0].spans[0].content, "hello");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn text_preview_keeps_enough_lines_for_scrolling() {
    let root = temp_path("scroll-depth");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("long.txt");
    let text = (1..=80)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&path, text).expect("failed to write long text");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Text);
    assert!(preview.lines.len() >= 80);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn utf8_preview_trims_to_last_valid_boundary() {
    let root = temp_path("utf8-boundary");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("unicode.txt");
    let bytes = [
        "a".repeat(PREVIEW_LIMIT_BYTES - 1).into_bytes(),
        "é".as_bytes().to_vec(),
    ]
    .concat();
    fs::write(&path, bytes).expect("failed to write unicode text");

    let preview = read_text_preview(&path)
        .expect("preview read should succeed")
        .expect("utf8 text should stay text");

    assert!(preview.bytes_truncated);
    assert_eq!(preview.text.len(), PREVIEW_LIMIT_BYTES - 1);
    assert!(preview.text.chars().all(|ch| ch == 'a'));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn utf8_text_file_is_not_mislabeled_as_binary() {
    let root = temp_path("utf8-text-kind");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("unicode.txt");
    let bytes = [
        "a".repeat(PREVIEW_LIMIT_BYTES - 1).into_bytes(),
        "é".as_bytes().to_vec(),
    ]
    .concat();
    fs::write(&path, bytes).expect("failed to write unicode text");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Text);
    assert!(preview.truncated);
    assert!(preview.lines.iter().all(|line| {
        line.spans
            .iter()
            .all(|span| span.content != "No text preview available")
    }));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn utf16le_bom_text_file_is_not_mislabeled_as_binary() {
    let root = temp_path("utf16le-text-kind");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("unicode.txt");
    let text = "Thu Jan 15 21:36:25 2026\r\nHello from UTF-16\r\n";
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    fs::write(&path, bytes).expect("failed to write utf16 text");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_ne!(preview.kind, PreviewKind::Binary);
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Hello from UTF-16"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn utf16_log_preview_uses_decoded_text() {
    let root = temp_path("utf16-log");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("socialclub.log");
    let text = "[00000000] Thu Jan 15 21:36:25 2026 INFO launcher started\r\n\
             [00000001] Thu Jan 15 21:36:26 2026 ERROR request_id=42 failed\r\n";
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    fs::write(&path, bytes).expect("failed to write utf16 log");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_ne!(preview.kind, PreviewKind::Binary);
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("launcher started") || line.contains("request_id=42"))
    );
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Log"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn byte_truncated_preview_reports_truncation_without_fake_line_totals() {
    let root = temp_path("byte-truncated");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("notes.txt");
    fs::write(&path, "a".repeat(PREVIEW_LIMIT_BYTES + 32)).expect("failed to write text");

    let preview = build_preview(&file_entry(path));
    let header = preview
        .header_detail(0, 20)
        .expect("header detail should be present");

    assert_eq!(preview.kind, PreviewKind::Text);
    assert!(preview.truncated);
    assert!(preview.source_lines.is_none());
    assert!(header.contains("truncated to 64 KiB"));
    assert!(!header.contains("lines"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn line_truncated_preview_reports_visible_limit() {
    let root = temp_path("line-truncated");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("long.txt");
    let total_lines = PREVIEW_RENDER_LINE_LIMIT + 40;
    let text = (1..=total_lines)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&path, text).expect("failed to write long text");

    let preview = build_preview(&file_entry(path));
    let header = preview
        .header_detail(0, 20)
        .expect("header detail should be present");

    assert!(preview.truncated);
    assert_eq!(preview.source_lines, Some(total_lines));
    assert!(header.contains(&format!("{total_lines} lines")));
    assert!(header.contains(&format!("showing first {PREVIEW_RENDER_LINE_LIMIT} lines")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
#[cfg(unix)]
fn protected_directory_preview_reports_permission_denied() {
    // Skip when running as root (e.g. FreeBSD CI) — root bypasses chmod 000.
    if unsafe { libc::getuid() } == 0 {
        return;
    }

    let root = temp_path("protected-dir-preview");
    let locked = root.join("locked");
    fs::create_dir_all(&locked).expect("failed to create locked dir");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("failed to lock dir");

    let preview = build_preview(&directory_entry(locked.clone()));

    assert_eq!(preview.kind, PreviewKind::Unavailable);
    assert_eq!(preview.detail.as_deref(), Some("Permission denied"));
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line_text(line).contains("permission"))
    );

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("failed to unlock dir");
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
#[cfg(unix)]
fn protected_file_preview_reports_permission_denied() {
    // Skip when running as root (e.g. FreeBSD CI) — root bypasses chmod 000.
    if unsafe { libc::getuid() } == 0 {
        return;
    }

    let root = temp_path("protected-file-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("secret.txt");
    fs::write(&path, "secret").expect("failed to write file");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).expect("failed to lock file");

    let preview = build_preview(&file_entry(path.clone()));

    assert_eq!(preview.kind, PreviewKind::Unavailable);
    assert_eq!(preview.detail.as_deref(), Some("Permission denied"));
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line_text(line).contains("permission"))
    );

    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("failed to unlock file");
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn license_file_with_hard_line_breaks_is_reflowed_into_paragraphs() {
    let root = temp_path("license-reflow");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("LICENSE");

    // Simulate a hard-wrapped license (lines at ~76 chars, traditional terminal format).
    // Each paragraph is a block of consecutive lines, separated by blank lines.
    let contents = "\
MIT License

Copyright (c) 2024 Example Author

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the \"Software\"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.
";
    fs::write(&path, contents).expect("failed to write license");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Text);

    // After reflowing: the five-line permission-grant block should be ONE long line.
    let permission_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("Permission is hereby granted"))
        .expect("permission grant line should exist");
    let text = line_text(permission_line);
    // The reflowed line must contain the last part that was originally on a separate line.
    assert!(
        text.contains("furnished to do so"),
        "permission grant should be reflowed into a single line, got: {text:?}"
    );

    // Blank-line paragraph separators must be preserved.
    let blank_count = preview.lines.iter().filter(|l| l.spans.is_empty()).count();
    assert!(
        blank_count >= 3,
        "blank paragraph separators should be preserved, got {blank_count}"
    );

    // The reflowed preview must have far fewer lines than the source (paragraphs, not raw lines).
    let source_lines = contents.lines().count();
    assert!(
        preview.lines.len() < source_lines,
        "reflowed output ({} lines) should be shorter than source ({source_lines} lines)",
        preview.lines.len()
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
