use super::*;

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
