use super::*;

#[test]
fn desktop_entry_renderer_handles_unicode_values() {
    let lines = render_built_in_code_preview(
        CustomCodeKind::DesktopEntry,
        "[Desktop Entry]\nName=エリオ\nName[ja]=日本語アプリ\n",
        true,
        20,
        &|| false,
    );

    assert!(
        lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("日本語アプリ"))
    );
}
