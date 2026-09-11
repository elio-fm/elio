use super::*;

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
