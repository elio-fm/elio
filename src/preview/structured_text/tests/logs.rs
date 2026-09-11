use super::*;

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
