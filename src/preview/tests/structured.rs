use super::*;

#[test]
fn toml_preview_uses_structured_renderer() {
    let root = temp_path("toml");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("config.toml");
    fs::write(
        &path,
        "[package]\nname = \"elio\"\nversion = \"0.1.0\"\n\n[server]\nport = 3000\n",
    )
    .expect("failed to write toml");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("TOML"));
    let lines = preview
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    assert!(lines.iter().any(|line| line.contains("[package]")));
    assert!(lines.iter().any(|line| line.contains("name = \"elio\"")));
    assert!(lines.iter().any(|line| line.contains("[server]")));
    assert!(lines.iter().any(|line| line.contains("port = 3000")));
    assert!(!lines.iter().any(|line| line.contains("root: object")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn log_preview_uses_structured_renderer() {
    let root = temp_path("log");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("server.log");
    fs::write(
        &path,
        "2026-03-10T12:00:00Z ERROR request_id=42 path=/login failed\n",
    )
    .expect("failed to write log");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Log"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("ERROR"))
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("request_id"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn multiline_log_preview_keeps_stack_trace_context() {
    let root = temp_path("log-multiline");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("server.log");
    fs::write(
        &path,
        "2026-03-10T12:00:00Z ERROR request_id=42 msg=\"request failed\"\n\
             \tat service.handle (/srv/app.js:10)\n\
             Caused by: timeout\n\
             2026-03-10T12:00:01Z INFO request_id=42 recovered\n",
    )
    .expect("failed to write log");

    let preview = build_preview(&file_entry(path));
    let rendered = preview
        .lines
        .iter()
        .map(Line::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Log"));
    assert!(rendered.contains("request failed"));
    assert!(rendered.contains("Caused by: timeout"));
    assert!(rendered.contains("recovered"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn unstructured_log_preview_uses_log_highlighting() {
    let root = temp_path("log-highlighting");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("notes.log");
    fs::write(
        &path,
        "starting application\nloading configuration\nready\n",
    )
    .expect("failed to write log");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Log file"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("starting application"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

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

#[test]
fn dotenv_preview_uses_structured_renderer() {
    let root = temp_path("dotenv");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join(".env.local");
    fs::write(&path, "APP_ENV=dev\nPORT=3000\n").expect("failed to write dotenv file");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail == ".env")
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("APP_ENV"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn jsonc_preview_uses_structured_renderer() {
    let root = temp_path("jsonc");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("deno.jsonc");
    fs::write(&path, "{\n  // comment\n  \"name\": \"elio\",\n}\n").expect("failed to write jsonc");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("JSONC"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("name"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn json5_preview_uses_structured_renderer() {
    let root = temp_path("json5");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("config.json5");
    fs::write(&path, "{\n  trailing: true,\n  list: [1, 2,],\n}\n").expect("failed to write json5");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("JSON5"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("trailing"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn yaml_preview_uses_structured_renderer() {
    let root = temp_path("yaml");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("docker-compose.yaml");
    fs::write(
        &path,
        "services:\n  app:\n    image: elio:latest\n    ports:\n      - \"3000:3000\"\n",
    )
    .expect("failed to write yaml");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("YAML"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("services"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
