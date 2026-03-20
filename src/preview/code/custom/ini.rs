use super::{split_unquoted_once, styled_text};
use crate::ui::theme;
use ratatui::{style::Modifier, text::Span};

pub(super) fn highlight_ini_line(
    line: &str,
    palette: theme::CodePreviewPalette,
    desktop_entry_mode: bool,
) -> Vec<Span<'static>> {
    let trimmed = line.trim_start();
    let indent = &line[..line.len().saturating_sub(trimmed.len())];

    if trimmed.starts_with('#') || trimmed.starts_with(';') {
        return vec![
            Span::raw(indent.to_string()),
            styled_text(trimmed, palette.comment, Modifier::ITALIC),
        ];
    }

    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        let color = if desktop_entry_mode && trimmed == "[Desktop Entry]" {
            palette.keyword
        } else {
            palette.r#type
        };
        return vec![
            Span::raw(indent.to_string()),
            styled_text(trimmed, color, Modifier::BOLD),
        ];
    }

    if let Some((left, right)) = split_unquoted_once(trimmed, '=') {
        let key = left.trim_end();
        let key_color = if desktop_entry_mode
            && matches!(
                key,
                "Name" | "Exec" | "Icon" | "Type" | "Terminal" | "Categories"
            ) {
            palette.function
        } else {
            palette.parameter
        };
        let mut spans = vec![
            Span::raw(indent.to_string()),
            styled_text(key, key_color, Modifier::BOLD),
            styled_text("=", palette.operator, Modifier::empty()),
        ];
        if !right.is_empty() {
            spans.extend(super::data::highlight_value_fragment(right, palette));
        }
        return spans;
    }

    super::data::highlight_value_fragment(line, palette)
}
