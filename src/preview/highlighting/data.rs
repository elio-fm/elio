use super::common::{
    looks_numeric, scan_quoted_segment, split_comment, split_jsonc_segments, split_unquoted_once,
    styled_text,
};
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
            spans.extend(highlight_value_fragment(right, palette));
        }
        return spans;
    }

    highlight_value_fragment(line, palette)
}

pub(super) fn highlight_toml_line(
    line: &str,
    palette: theme::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let trimmed = line.trim_start();
    let indent = &line[..line.len().saturating_sub(trimmed.len())];

    if trimmed.starts_with('#') {
        return vec![
            Span::raw(indent.to_string()),
            styled_text(trimmed, palette.comment, Modifier::ITALIC),
        ];
    }

    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return vec![
            Span::raw(indent.to_string()),
            styled_text(trimmed, palette.r#type, Modifier::BOLD),
        ];
    }

    if let Some((left, right)) = split_unquoted_once(trimmed, '=') {
        let mut spans = vec![
            Span::raw(indent.to_string()),
            styled_text(left.trim_end(), palette.function, Modifier::BOLD),
            styled_text(" = ", palette.operator, Modifier::empty()),
        ];
        spans.extend(highlight_value_fragment(right.trim_start(), palette));
        return spans;
    }

    highlight_value_fragment(line, palette)
}

pub(super) fn highlight_json_line(
    line: &str,
    palette: theme::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut index = 0usize;

    while index < line.len() {
        let ch = line[index..].chars().next().unwrap_or(' ');
        if ch.is_whitespace() {
            let start = index;
            while let Some(current) = line[index..].chars().next() {
                if !current.is_whitespace() {
                    break;
                }
                index += current.len_utf8();
            }
            spans.push(Span::raw(line[start..index].to_string()));
            continue;
        }

        if ch == '"' {
            let end = scan_quoted_segment(line, index);
            let token = &line[index..end];
            let next = line[end..].chars().find(|c| !c.is_whitespace());
            let color = if next == Some(':') {
                palette.function
            } else {
                palette.string
            };
            spans.push(styled_text(token, color, Modifier::empty()));
            index = end;
            continue;
        }

        if "{}[]:,".contains(ch) {
            spans.push(styled_text(
                &line[index..index + ch.len_utf8()],
                palette.operator,
                Modifier::empty(),
            ));
            index += ch.len_utf8();
            continue;
        }

        let start = index;
        while let Some(current) = line[index..].chars().next() {
            if current.is_whitespace() || "{}[]:,".contains(current) {
                break;
            }
            index += current.len_utf8();
        }
        spans.extend(highlight_scalar_token(&line[start..index], palette));
    }

    spans
}

pub(super) fn highlight_jsonc_line(
    line: &str,
    palette: theme::CodePreviewPalette,
    in_block_comment: &mut bool,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    for (is_comment, segment) in split_jsonc_segments(line, in_block_comment) {
        if is_comment {
            spans.push(styled_text(segment, palette.comment, Modifier::ITALIC));
        } else {
            spans.extend(highlight_json_line(segment, palette));
        }
    }

    spans
}

pub(super) fn highlight_yaml_line(
    line: &str,
    palette: theme::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let (body, comment) = split_comment(line);
    let trimmed = body.trim_start();
    let indent = &body[..body.len().saturating_sub(trimmed.len())];
    let mut spans = vec![Span::raw(indent.to_string())];
    let content = if let Some(rest) = trimmed.strip_prefix("- ") {
        spans.push(styled_text("- ", palette.operator, Modifier::empty()));
        rest
    } else {
        trimmed
    };

    if let Some((left, right)) = split_unquoted_once(content, ':') {
        spans.push(styled_text(
            left.trim_end(),
            palette.function,
            Modifier::BOLD,
        ));
        spans.push(styled_text(":", palette.operator, Modifier::empty()));
        if !right.is_empty() {
            spans.push(Span::raw(" ".to_string()));
            spans.extend(highlight_value_fragment(right.trim_start(), palette));
        }
    } else {
        spans.extend(highlight_value_fragment(content, palette));
    }

    if let Some(comment) = comment {
        if !body.is_empty() {
            spans.push(Span::raw(" ".to_string()));
        }
        spans.push(styled_text(comment, palette.comment, Modifier::ITALIC));
    }

    spans
}

pub(super) fn highlight_value_fragment(
    value: &str,
    palette: theme::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let (body, comment) = split_comment(value);
    let mut spans = highlight_token_stream(body, palette);
    if let Some(comment) = comment {
        if !body.is_empty() {
            spans.push(Span::raw(" ".to_string()));
        }
        spans.push(styled_text(comment, palette.comment, Modifier::ITALIC));
    }
    spans
}

fn highlight_token_stream(input: &str, palette: theme::CodePreviewPalette) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut index = 0usize;

    while let Some(ch) = input[index..].chars().next() {
        if ch.is_whitespace() {
            let start = index;
            while let Some(current) = input[index..].chars().next() {
                if !current.is_whitespace() {
                    break;
                }
                index += current.len_utf8();
            }
            spans.push(Span::raw(input[start..index].to_string()));
            continue;
        }

        if ch == '"' || ch == '\'' {
            let end = scan_quoted_segment(input, index);
            spans.push(styled_text(
                &input[index..end],
                palette.string,
                Modifier::empty(),
            ));
            index = end;
            continue;
        }

        if "[]{}(),:".contains(ch) {
            let end = index + ch.len_utf8();
            spans.push(styled_text(
                &input[index..end],
                palette.operator,
                Modifier::empty(),
            ));
            index = end;
            continue;
        }

        let start = index;
        while let Some(current) = input[index..].chars().next() {
            if current.is_whitespace()
                || "[]{}(),:#".contains(current)
                || current == '"'
                || current == '\''
            {
                break;
            }
            index += current.len_utf8();
        }
        spans.extend(highlight_scalar_token(&input[start..index], palette));
    }

    spans
}

fn highlight_scalar_token(token: &str, palette: theme::CodePreviewPalette) -> Vec<Span<'static>> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return vec![Span::raw(token.to_string())];
    }

    let color = if matches!(trimmed, "true" | "false" | "null") {
        palette.keyword
    } else if looks_numeric(trimmed) {
        palette.constant
    } else {
        palette.fg
    };

    vec![styled_text(token, color, Modifier::empty())]
}
