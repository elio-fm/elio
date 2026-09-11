use super::*;

#[test]
fn jar_preview_surfaces_manifest_metadata() {
    let root = temp_path("jar-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("app.jar");
    write_zip_entries(
        &path,
        &[
            (
                "META-INF/MANIFEST.MF",
                "Implementation-Title: Elio\nImplementation-Version: 1.2.3\nMain-Class: elio.Main\nCreated-By: OpenJDK\n",
            ),
            ("elio/Main.class", "compiled"),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("Java archive"));
    assert!(line_texts.iter().any(|text| text == "Manifest"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Title") && text.contains("Elio"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Version") && text.contains("1.2.3"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Main-Class") && text.contains("elio.Main"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
