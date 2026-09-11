use super::*;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn font_preview_uses_metadata_details_instead_of_binary_placeholder() {
    let root = temp_path("font-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");

    for (file_name, detail, expected_format, bytes) in [
        (
            "demo.ttf",
            "TrueType font",
            "TrueType",
            &b"\x00\x01\x00\x00rest"[..],
        ),
        (
            "demo.otf",
            "OpenType font",
            "OpenType (CFF)",
            &b"OTTOrest"[..],
        ),
        (
            "demo.woff",
            "WOFF font",
            "WOFF (TrueType)",
            &b"wOFF\x00\x01\x00\x00"[..],
        ),
        (
            "demo.woff2",
            "WOFF2 font",
            "WOFF2 (TrueType)",
            &b"wOF2\x00\x01\x00\x00"[..],
        ),
    ] {
        let path = root.join(file_name);
        fs::write(&path, bytes).expect("failed to write font fixture");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Font);
        assert_eq!(preview.section_label(), "Font");
        assert_eq!(preview.detail.as_deref(), Some(detail));
        assert_eq!(line_texts.first().map(String::as_str), Some("Details"));
        assert!(
            line_texts
                .iter()
                .any(|line| line.contains("Format") && line.contains(expected_format))
        );
        assert!(line_texts.iter().any(|line| line.contains("File Size")));
        assert!(
            line_texts
                .iter()
                .all(|line| !line.contains("Binary or unsupported file"))
        );
        assert!(line_texts.iter().all(|line| !line.contains("PostScript")));
        assert!(
            line_texts
                .iter()
                .all(|line| !line.contains("No text preview available"))
        );
        assert!(
            line_texts
                .iter()
                .all(|line| !line.contains("Loading preview"))
        );
    }

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn font_loading_preview_stays_silent() {
    let root = temp_path("font-loading");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("demo.ttf");
    fs::write(&path, b"\x00\x01\x00\x00rest").expect("failed to write font fixture");

    let preview = loading_preview_for(&file_entry(path), &PreviewRequestOptions::Default);

    assert_eq!(preview.kind, PreviewKind::Font);
    assert_eq!(preview.section_label(), "Font");
    assert_eq!(preview.detail.as_deref(), Some("TrueType font"));
    assert!(preview.lines.is_empty());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
#[cfg(unix)]
fn protected_font_preview_reports_permission_denied() {
    // Skip when running as root (e.g. FreeBSD CI) — root bypasses chmod 000.
    if unsafe { libc::getuid() } == 0 {
        return;
    }

    let root = temp_path("protected-font-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("secret.ttf");
    fs::write(&path, b"\x00\x01\x00\x00rest").expect("failed to write font fixture");
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
