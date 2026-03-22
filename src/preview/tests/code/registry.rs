use super::*;

#[test]
fn generic_lockfile_uses_code_renderer() {
    let root = temp_path("lock");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("deps.lock");
    fs::write(&path, "[packages]\nelio=1.0.0\n").expect("failed to write lockfile");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Lockfile"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("elio"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn makefile_preview_uses_code_renderer() {
    let root = temp_path("makefile");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("Makefile");
    fs::write(
        &path,
        "CC := clang\n.PHONY: build\nbuild: main.o util.o\n\t$(CC) -o app main.o util.o\n",
    )
    .expect("failed to write makefile");

    let preview = build_preview(&file_entry(path.clone()));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Make"))
    );
    assert!(line_texts.iter().any(|text| text.contains(".PHONY")));
    assert!(line_texts.iter().any(|text| text.contains("$(CC)")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn html_preview_uses_code_renderer() {
    let root = temp_path("html");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("index.html");
    fs::write(
        &path,
        "<!DOCTYPE html>\n<div class=\"app\" data-id=\"42\">elio</div>\n",
    )
    .expect("failed to write html");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some());
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("div"))
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("class"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn css_preview_uses_code_renderer() {
    let root = temp_path("css");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("styles.css");
    fs::write(&path, ".app {\n  color: #fff;\n  margin: 12px;\n}\n").expect("failed to write css");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some());
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("color"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn xml_preview_uses_code_renderer() {
    let root = temp_path("xml");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("layout.xml");
    fs::write(&path, "<?xml version=\"1.0\"?>\n<layout id=\"main\" />\n")
        .expect("failed to write xml");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some());
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("layout"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cargo_lock_preview_uses_code_renderer() {
    let root = temp_path("cargo-lock");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("Cargo.lock");
    fs::write(&path, "version = 3\n").expect("failed to write cargo lock");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("TOML"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
