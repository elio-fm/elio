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
