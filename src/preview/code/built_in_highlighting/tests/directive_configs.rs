use super::*;

#[test]
fn directive_renderer_keeps_existing_palette_contract() {
    let palette = theme::code_preview_palette();
    let lines = render_built_in_code_preview(
        CustomCodeKind::DirectiveConf,
        "font_size 11.5\nforeground #c0c6e2\ninclude ~/.config/kitty/theme.conf\n",
        true,
        20,
        &|| false,
    );

    assert_span_color(&lines[0], "font_size", palette.function);
    assert_span_color(&lines[0], "11.5", palette.constant);
    assert_span_color(&lines[1], "foreground", palette.function);
    assert_span_color(&lines[1], "#c0c6e2", palette.constant);
    assert_span_color(&lines[2], "include", palette.function);
    assert_span_color(&lines[2], "~/.config/kitty/theme.conf", palette.string);
}
