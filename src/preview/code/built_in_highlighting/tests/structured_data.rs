use super::*;

#[test]
fn jsonc_renderer_keeps_line_comments_and_block_comments() {
    let lines = render_built_in_code_preview(
        CustomCodeKind::Jsonc,
        "{\n  // comment\n  /* block */\n  \"name\": \"elio\"\n}\n",
        true,
        20,
        &|| false,
    );

    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("// comment"))
    );
    assert!(
        lines[2]
            .spans
            .iter()
            .any(|span| span.content.contains("/* block */"))
    );
    assert!(line_text(&lines[3]).contains("\"name\": \"elio\""));
}
