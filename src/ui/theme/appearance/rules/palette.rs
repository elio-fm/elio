use super::super::types::{CodePreviewPalette, Palette, PreviewTheme};
use super::shared::rgb;

pub(super) fn default_palette() -> Palette {
    Palette {
        bg: rgb(2, 5, 12),
        chrome: rgb(7, 13, 22),
        chrome_alt: rgb(11, 18, 32),
        chip_text: rgb(12, 12, 12),
        panel: rgb(9, 16, 27),
        panel_alt: rgb(6, 11, 20),
        surface: rgb(16, 25, 42),
        elevated: rgb(21, 32, 54),
        border: rgb(53, 80, 111),
        text: rgb(237, 244, 255),
        muted: rgb(142, 162, 191),
        accent: rgb(126, 196, 255),
        accent_soft: rgb(20, 54, 87),
        accent_text: rgb(234, 245, 255),
        selected_bg: rgb(32, 64, 100),
        selected_border: rgb(149, 211, 255),
        selection_bar: rgb(255, 178, 86),
        yank_bar: rgb(87, 201, 87),
        cut_bar: rgb(224, 90, 90),
        grid_selection_band: rgb(52, 40, 18),
        grid_yank_band: rgb(18, 44, 20),
        grid_cut_band: rgb(48, 20, 20),
        trash_bar: rgb(210, 65, 95),
        restore_bar: rgb(65, 160, 220),
        sidebar_active: rgb(27, 56, 88),
        button_bg: rgb(14, 23, 38),
        button_disabled_bg: rgb(8, 16, 27),
        path_bg: rgb(12, 19, 32),
    }
}

pub(super) fn default_preview_theme() -> PreviewTheme {
    PreviewTheme {
        code: CodePreviewPalette {
            fg: rgb(215, 227, 244),
            bg: rgb(10, 13, 18),
            selection_bg: rgb(18, 42, 63),
            selection_fg: rgb(242, 247, 255),
            caret: rgb(18, 210, 255),
            line_highlight: rgb(16, 21, 31),
            line_number: rgb(123, 144, 167),
            comment: rgb(111, 131, 153),
            string: rgb(121, 231, 213),
            constant: rgb(255, 166, 87),
            keyword: rgb(255, 120, 198),
            function: rgb(54, 215, 255),
            r#type: rgb(179, 140, 255),
            parameter: rgb(255, 216, 102),
            tag: rgb(89, 222, 148),
            operator: rgb(138, 231, 255),
            r#macro: rgb(255, 143, 64),
            invalid: rgb(255, 133, 133),
        },
    }
}
