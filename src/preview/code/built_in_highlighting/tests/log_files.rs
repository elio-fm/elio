use super::*;

#[test]
fn log_renderer_highlights_levels_and_fields() {
    let lines = render_built_in_code_preview(
        CustomCodeKind::Log,
        "2026-03-10T12:00:00Z ERROR request_id=42 path=/login failed\n",
        true,
        20,
        &|| false,
    );

    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("ERROR"))
    );
    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("request_id"))
    );
}
