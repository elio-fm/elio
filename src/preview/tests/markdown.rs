use super::*;

#[test]
fn markdown_preview_formats_headings_and_lists() {
    let root = temp_path("markdown");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(&path, "# Heading\n- item\n`inline`\n").expect("failed to write markdown");

    let preview = build_preview(&file_entry(path.clone()));

    assert_eq!(preview.kind, PreviewKind::Markdown);
    assert_eq!(preview.lines[0].spans[0].content, "Heading");
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content == "inline"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_formats_inline_emphasis_mid_line() {
    let root = temp_path("markdown-inline");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(&path, "hello **bold** world\n").expect("failed to write markdown");

    let preview = build_preview(&file_entry(path.clone()));
    let line = &preview.lines[0];

    assert_eq!(preview.kind, PreviewKind::Markdown);
    assert!(line.spans.iter().any(|span| span.content == "hello "));
    assert!(line.spans.iter().any(|span| span.content == "bold"));
    assert!(line.spans.iter().any(|span| span.content == " world"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_renders_fenced_code_blocks() {
    let root = temp_path("markdown-fence");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(&path, "```rust\nfn main() {}\n```\n").expect("failed to write markdown");

    let preview = build_preview(&file_entry(path.clone()));

    assert_eq!(preview.kind, PreviewKind::Markdown);
    assert_eq!(preview.lines[0].spans[1].content, "rust");
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line_text(line).contains("fn main() {}"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_routes_fence_aliases_through_registry() {
    let root = temp_path("markdown-fence-aliases");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(
        &path,
        "```js\nconst value = 1;\n```\n\n```csharp\npublic class Greeter {}\n```\n\n```exs\ndefmodule Greeter do\nend\n```\n\n```clj\n(defn greet [name] (str \"hi \" name))\n```\n\n```pwsh\nfunction Invoke-Greeting { Write-Host \"hello\" }\n```\n\n```kitty\nfont_size 11.5\n```\n",
    )
    .expect("failed to write markdown");

    let preview = build_preview(&file_entry(path.clone()));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Markdown);
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content == "js"))
    );
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content == "kitty"))
    );

    let js_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("const value = 1;"))
        .expect("expected highlighted js line");
    assert_ne!(span_color(js_line, "const"), Some(code_palette.fg));

    let kitty_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("font_size 11.5"))
        .expect("expected highlighted kitty line");
    assert_eq!(
        span_color(kitty_line, "font_size"),
        Some(code_palette.function)
    );

    let csharp_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("public class Greeter {}"))
        .expect("expected highlighted csharp line");
    assert_ne!(span_color(csharp_line, "public"), Some(code_palette.fg));

    let elixir_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("defmodule Greeter do"))
        .expect("expected highlighted elixir line");
    assert_ne!(span_color(elixir_line, "defmodule"), Some(code_palette.fg));

    let powershell_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("function Invoke-Greeting"))
        .expect("expected highlighted powershell line");
    assert_ne!(
        span_color(powershell_line, "function"),
        Some(code_palette.fg)
    );

    let clojure_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("(defn greet [name]"))
        .expect("expected highlighted clojure line");
    assert_ne!(span_color(clojure_line, "defn"), Some(code_palette.fg));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_renders_links() {
    let root = temp_path("markdown-links");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(&path, "open [elio](https://example.com)\n").expect("failed to write markdown");

    let preview = build_preview(&file_entry(path));
    let line = &preview.lines[0];

    assert_eq!(preview.kind, PreviewKind::Markdown);
    let link_span = line
        .spans
        .iter()
        .find(|span| span.content == "elio")
        .expect("link label should be rendered");
    assert!(link_span.style.add_modifier.contains(Modifier::UNDERLINED));
    assert!(line_text(line).contains("(https://example.com)"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_renders_images_with_icon_and_alt_text() {
    let root = temp_path("markdown-images");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(
        &path,
        "![Catppuccin Mocha](examples/themes/catppuccin-mocha/screenshot.png)\n",
    )
    .expect("failed to write markdown");

    let preview = build_preview(&file_entry(path.clone()));

    assert_eq!(preview.kind, PreviewKind::Markdown);
    let texts: Vec<String> = preview.lines.iter().map(line_text).collect();
    assert!(texts.iter().any(|t| t.contains("󰋩 Catppuccin Mocha")));
    // Alt text shown
    assert!(texts.iter().any(|t| t.contains("Catppuccin Mocha")));
    // Path not shown
    assert!(!texts.iter().any(|t| t.contains("screenshot.png")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_adds_spacing_between_blocks() {
    let root = temp_path("markdown-spacing");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(
        &path,
        "# Heading\nParagraph text\n\n```rust\nlet x = 1;\n```\n",
    )
    .expect("failed to write markdown");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Markdown);
    assert!(preview.lines.iter().any(|line| line.spans.is_empty()));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_renders_nested_emphasis() {
    let root = temp_path("markdown-nested");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(&path, "**bold and *italic***\n").expect("failed to write markdown");

    let preview = build_preview(&file_entry(path));
    let line = &preview.lines[0];

    assert_eq!(preview.kind, PreviewKind::Markdown);
    let italic_span = line
        .spans
        .iter()
        .find(|span| span.content == "italic")
        .expect("nested italic content should be rendered");
    assert!(italic_span.style.add_modifier.contains(Modifier::BOLD));
    assert!(italic_span.style.add_modifier.contains(Modifier::ITALIC));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_renders_mixed_lists() {
    let root = temp_path("markdown-mixed-lists");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(&path, "1. first\n   - nested\n2. second\n").expect("failed to write markdown");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Markdown);
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content == "1. "))
    );
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content.contains("• ")))
    );
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content == "2. "))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_renders_table_with_header_separator() {
    let root = temp_path("markdown-table");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(
        &path,
        "| Key | Description |\n|---|---|\n| Enter | Open |\n| Backspace | Go up |\n",
    )
    .expect("failed to write markdown");

    let preview = build_preview(&file_entry(path.clone()));

    assert_eq!(preview.kind, PreviewKind::Markdown);
    let texts: Vec<String> = preview.lines.iter().map(line_text).collect();
    // Box borders are present
    assert!(
        texts.iter().any(|t| t.contains('┌')),
        "expected top-left corner"
    );
    assert!(
        texts.iter().any(|t| t.contains('├')),
        "expected header separator"
    );
    assert!(
        texts.iter().any(|t| t.contains('└')),
        "expected bottom-left corner"
    );
    assert!(
        texts.iter().any(|t| t.contains('│')),
        "expected vertical border"
    );
    // Header row content visible
    assert!(
        texts
            .iter()
            .any(|t| t.contains("Key") && t.contains("Description"))
    );
    // Data rows visible
    assert!(
        texts
            .iter()
            .any(|t| t.contains("Enter") && t.contains("Open"))
    );
    assert!(
        texts
            .iter()
            .any(|t| t.contains("Backspace") && t.contains("Go up"))
    );
    // Header cells are bold
    let header_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("Key") && line_text(line).contains("Description"))
        .expect("expected header line");
    let key_span = header_line
        .spans
        .iter()
        .find(|span| span.content.contains("Key"))
        .expect("expected Key span");
    assert!(
        key_span
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_wraps_long_table_cells_without_ellipsis() {
    let root = temp_path("markdown-table-wrapping");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(
        &path,
        "| Key | Description |\n|---|---|\n| Enter | This description is deliberately longer than the markdown table preview column width so we can verify clipping behavior |\n",
    )
    .expect("failed to write markdown");

    let preview = build_preview(&file_entry(path.clone()));

    assert_eq!(preview.kind, PreviewKind::Markdown);
    let texts: Vec<String> = preview.lines.iter().map(line_text).collect();
    assert!(
        texts.iter().any(|t| t.contains("This description")),
        "expected wrapped table cell content to keep the leading text"
    );
    assert!(
        texts.iter().any(|t| t.contains("behavior")),
        "expected wrapped table cell content to keep the trailing text"
    );
    assert!(
        !texts.iter().any(|t| t.contains('…')),
        "wrapped table cells should not render an ellipsis"
    );
    assert!(
        texts.iter().filter(|t| t.contains('│')).count() > 2,
        "expected the long table row to wrap across multiple table lines"
    );
    let top_border = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains('┌'))
        .expect("expected table top border");
    assert_eq!(
        top_border.width(),
        MARKDOWN_CONTENT_WIDTH,
        "wide markdown tables should use the same width budget as markdown prose"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_renders_details_summary() {
    let root = temp_path("markdown-details");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(
        &path,
        "<details>\n<summary><strong>Controls and Navigation</strong></summary>\n\nSome content here.\n\n</details>\n",
    )
    .expect("failed to write markdown");

    let preview = build_preview(&file_entry(path.clone()));

    assert_eq!(preview.kind, PreviewKind::Markdown);
    let texts: Vec<String> = preview.lines.iter().map(line_text).collect();
    // Summary text is shown (tags stripped)
    assert!(texts.iter().any(|t| t.contains("Controls and Navigation")));
    // Bare <details> tags are not shown as raw HTML
    assert!(!texts.iter().any(|t| t.trim() == "<details>"));
    assert!(!texts.iter().any(|t| t.trim() == "</details>"));
    // The disclosure indicator is present
    assert!(texts.iter().any(|t| t.contains('▶')));
    // Body content is shown with gutter prefix
    let content_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("Some content here."))
        .expect("expected content line");
    assert!(
        content_line
            .spans
            .iter()
            .any(|span| span.content.contains('╎')),
        "content inside details should have gutter"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_license_preview_keeps_detected_detail() {
    let root = temp_path("markdown-license");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("LICENSE.md");
    fs::write(
        &path,
        "# SPDX-License-Identifier: Apache-2.0\n\nFixture license notes.\n",
    )
    .expect("failed to write markdown license");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Markdown);
    assert_eq!(preview.detail.as_deref(), Some("Apache License 2.0"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
