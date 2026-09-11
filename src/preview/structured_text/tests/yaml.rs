use super::*;

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
