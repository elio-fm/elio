use super::*;

#[test]
fn json_preview_formats_minified_content() {
    let root = temp_path("json");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("package.json");
    fs::write(&path, "{\"name\":\"elio\",\"nested\":{\"enabled\":true}}\n")
        .expect("failed to write json");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("JSON"));
    assert_eq!(preview.source_lines, Some(1));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("nested"))
    );
    assert!(preview.lines.len() > 1);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn json_preview_adds_root_summary_and_array_indexes() {
    let root = temp_path("json-summary");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("data.json");
    fs::write(&path, "{\"items\":[{\"id\":1},{\"id\":2}],\"ok\":true}\n")
        .expect("failed to write json");

    let preview = build_preview(&file_entry(path));
    let rendered = preview
        .lines
        .iter()
        .map(Line::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("root: object"));
    assert!(rendered.contains("2 keys"));
    assert!(rendered.contains("[0]: {id: 1}"));
    assert!(rendered.contains("[1]: {id: 2}"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn json_preview_inlines_small_scalar_structures() {
    let root = temp_path("json-inline");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("data.json");
    fs::write(
        &path,
        "{\"meta\":{\"id\":1,\"env\":\"dev\"},\"ports\":[80,443]}\n",
    )
    .expect("failed to write json");

    let preview = build_preview(&file_entry(path));
    let rendered = preview
        .lines
        .iter()
        .map(Line::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("meta: {env: \"dev\", id: 1}"));
    assert!(rendered.contains("ports: [80, 443]"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn json_preview_truncates_long_strings_with_length_hint() {
    let root = temp_path("json-long-string");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("data.json");
    fs::write(&path, format!("{{\"token\":\"{}\"}}\n", "a".repeat(120)))
        .expect("failed to write json");

    let preview = build_preview(&file_entry(path));
    let rendered = preview
        .lines
        .iter()
        .map(Line::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("token: "));
    assert!(rendered.contains("(120 chars)"));
    assert!(rendered.contains("…"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn truncated_json_preview_reports_why_formatting_was_skipped() {
    let root = temp_path("json-truncated");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("package.json");
    let oversized = format!("{{\"value\":\"{}\"}}", "a".repeat(PREVIEW_LIMIT_BYTES));
    fs::write(&path, oversized).expect("failed to write oversized json");

    let preview = build_preview(&file_entry(path));
    let header = preview
        .header_detail(0, 12)
        .expect("formatted header detail should be present");

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        header.contains("formatted preview unavailable for partial file"),
        "unexpected header: {header}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
