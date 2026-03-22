use super::*;

#[test]
fn code_preview_includes_line_numbers() {
    let root = temp_path("code");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.rs");
    fs::write(&path, "fn main() {}\n").expect("failed to write code");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.lines[0].spans[0].content.contains("1"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

mod custom;
mod registry;
mod shell;
mod syntect;

#[test]
fn code_preview_sanitizes_control_characters() {
    let root = temp_path("control-char-code");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.c");
    let contents = "int main(void) {\n    puts(\"hello \u{1b} world\");\n    return 0;\n}\n";
    fs::write(&path, contents).expect("failed to write control-char source");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    assert!(
        line_texts.iter().any(|line| line.contains("^[ world")),
        "expected control characters to be rendered safely, got: {line_texts:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn code_preview_respects_custom_line_limit() {
    let root = temp_path("code-line-limit");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.rs");
    let text = (1..=12)
        .map(|index| format!("let value_{index} = {index};"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&path, text).expect("failed to write code");

    let preview = build_preview_with_options_and_code_line_limit(
        &file_entry(path),
        &PreviewRequestOptions::Default,
        4,
        false,
        false,
        &|| false,
    );
    let header = preview
        .header_detail(0, 20)
        .expect("header detail should be present");

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.lines.len(), 4);
    assert_eq!(
        preview.line_coverage.map(|coverage| coverage.shown_lines),
        Some(4)
    );
    assert!(
        header.contains("showing first 4 lines"),
        "unexpected header: {header}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
