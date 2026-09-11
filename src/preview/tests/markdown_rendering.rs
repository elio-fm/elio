use super::*;

fn line_text(line: &Line<'static>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
}

#[test]
fn renders_yaml_frontmatter_as_code_before_body() {
    let lines = render_markdown_preview(
        "---\ntitle: Daily note\ntags:\n  - obsidian\n---\n# Notes\nBody text",
    );
    let text: Vec<String> = lines.iter().map(line_text).collect();

    assert!(text.iter().any(|line| line.contains("yaml")));
    assert!(text.iter().any(|line| line.contains("title")));
    assert!(text.iter().any(|line| line.contains("obsidian")));
    assert!(text.iter().any(|line| line.contains("Notes")));
    assert!(!text.iter().any(|line| line.contains("────────────────")));
}

#[test]
fn only_treats_opening_delimited_block_as_frontmatter() {
    assert!(split_yaml_frontmatter("# Notes\n---\ntitle: no\n---\n").is_none());
    assert!(split_yaml_frontmatter("---\ntitle: no closing\n# Notes\n").is_none());

    let Some((frontmatter, body)) = split_yaml_frontmatter("---\ntitle: ok\n...\nBody") else {
        panic!("expected frontmatter");
    };
    assert_eq!(frontmatter, "title: ok");
    assert_eq!(body, "Body");
}
