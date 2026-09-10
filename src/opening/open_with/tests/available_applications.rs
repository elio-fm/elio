use super::*;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_dir(label: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-open-with-{label}-{unique}"))
}

#[test]
fn source_files_are_editor_compatible() {
    let root = temp_dir("text-like-source");
    fs::create_dir_all(&root).expect("create temp root");
    let path = root.join("main.rs");
    fs::write(&path, "fn main() {}\n").expect("write source file");

    assert!(is_editor_compatible(&path));

    fs::remove_dir_all(root).ok();
}

#[test]
fn svg_images_are_not_editor_compatible() {
    let root = temp_dir("text-like-svg");
    fs::create_dir_all(&root).expect("create temp root");
    let path = root.join("icon.svg");
    fs::write(
        &path,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"></svg>"#,
    )
    .expect("write svg file");

    assert!(!is_editor_compatible(&path));

    fs::remove_dir_all(root).ok();
}
