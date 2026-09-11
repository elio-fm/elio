use super::*;

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
