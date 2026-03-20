mod data;
mod directive;
mod ini;
mod logs;

use crate::{file_info::CustomCodeKind, ui::theme};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(crate) fn render_custom_code_preview<F>(
    kind: CustomCodeKind,
    text: &str,
    line_numbers: bool,
    line_limit: usize,
    canceled: &F,
) -> Vec<Line<'static>>
where
    F: Fn() -> bool,
{
    let code_palette = theme::code_preview_palette();
    let source_lines = crate::preview::collect_preview_lines_with_limit(
        text,
        crate::preview::clamp_code_preview_line_limit(line_limit),
    );
    let number_width = crate::preview::line_number_width(source_lines.len());
    let mut rendered = Vec::new();
    let mut jsonc_block_comment = false;

    for (index, line) in source_lines.iter().enumerate() {
        if canceled() {
            break;
        }

        let mut spans = Vec::new();
        if line_numbers {
            spans.push(crate::preview::line_number_span(index + 1, number_width));
        } else {
            spans.push(Span::styled(
                "│ ",
                Style::default().fg(code_palette.line_number),
            ));
        }

        let body = match kind {
            CustomCodeKind::DirectiveConf => {
                directive::highlight_directive_conf_line(line, code_palette)
            }
            CustomCodeKind::Ini => ini::highlight_ini_line(line, code_palette, false),
            CustomCodeKind::DesktopEntry => ini::highlight_ini_line(line, code_palette, true),
            CustomCodeKind::Json => data::highlight_json_line(line, code_palette),
            CustomCodeKind::Jsonc => {
                data::highlight_jsonc_line(line, code_palette, &mut jsonc_block_comment)
            }
            CustomCodeKind::Toml => data::highlight_toml_line(line, code_palette),
            CustomCodeKind::Yaml => data::highlight_yaml_line(line, code_palette),
            CustomCodeKind::Log => logs::highlight_log_line(line, code_palette),
        };
        spans.extend(body);
        rendered.push(Line::from(spans));
    }

    if rendered.is_empty() && !canceled() {
        rendered.push(Line::from("File is empty"));
    }

    rendered
}

pub(super) fn split_comment(input: &str) -> (&str, Option<&str>) {
    let mut in_string = false;
    let mut quote = '\0';
    let mut escape = false;

    for (index, ch) in input.char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            if ch == '\\' && quote == '"' {
                escape = true;
                continue;
            }
            if ch == quote {
                in_string = false;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            in_string = true;
            quote = ch;
            continue;
        }

        if ch == '#' {
            return (&input[..index], Some(&input[index..]));
        }
    }

    (input, None)
}

pub(super) fn split_unquoted_once(input: &str, needle: char) -> Option<(&str, &str)> {
    let mut in_string = false;
    let mut quote = '\0';
    let mut escape = false;

    for (index, ch) in input.char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            if ch == '\\' && quote == '"' {
                escape = true;
                continue;
            }
            if ch == quote {
                in_string = false;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            in_string = true;
            quote = ch;
            continue;
        }

        if ch == needle {
            let right_start = index + ch.len_utf8();
            return Some((&input[..index], &input[right_start..]));
        }
    }

    None
}

pub(super) fn split_jsonc_segments<'a>(
    line: &'a str,
    in_block_comment: &mut bool,
) -> Vec<(bool, &'a str)> {
    let mut segments = Vec::new();
    let mut cursor = 0usize;

    while cursor < line.len() {
        if *in_block_comment {
            let comment_start = cursor;
            if let Some(offset) = line[cursor..].find("*/") {
                let end = cursor + offset + 2;
                segments.push((true, &line[comment_start..end]));
                *in_block_comment = false;
                cursor = end;
            } else {
                segments.push((true, &line[comment_start..]));
                return segments;
            }
            continue;
        }

        let code_start = cursor;
        let mut index = cursor;
        let mut in_string = false;
        let mut escape = false;

        while index < line.len() {
            let ch = line[index..].chars().next().expect("valid utf-8 char");
            let next = index + ch.len_utf8();

            if in_string {
                if escape {
                    escape = false;
                    index = next;
                    continue;
                }
                if ch == '\\' {
                    escape = true;
                    index = next;
                    continue;
                }
                if ch == '"' {
                    in_string = false;
                }
                index = next;
                continue;
            }

            if ch == '"' {
                in_string = true;
                index = next;
                continue;
            }

            if ch == '/'
                && let Some(next_char) = line[next..].chars().next()
            {
                if next_char == '/' {
                    if code_start < index {
                        segments.push((false, &line[code_start..index]));
                    }
                    segments.push((true, &line[index..]));
                    return segments;
                }

                if next_char == '*' {
                    if code_start < index {
                        segments.push((false, &line[code_start..index]));
                    }

                    let comment_start = index;
                    let search_start = next + next_char.len_utf8();
                    if let Some(offset) = line[search_start..].find("*/") {
                        let end = search_start + offset + 2;
                        segments.push((true, &line[comment_start..end]));
                        cursor = end;
                    } else {
                        segments.push((true, &line[comment_start..]));
                        *in_block_comment = true;
                        return segments;
                    }
                    break;
                }
            }

            index = next;
        }

        if cursor == index {
            segments.push((false, &line[cursor..]));
            return segments;
        }

        if index >= line.len() {
            segments.push((false, &line[code_start..]));
            return segments;
        }
    }

    if segments.is_empty() {
        segments.push((false, line));
    }
    segments
}

pub(super) fn scan_quoted_segment(input: &str, start: usize) -> usize {
    let quote = input[start..].chars().next().unwrap_or('"');
    let mut index = start + quote.len_utf8();
    let mut escape = false;

    while let Some(ch) = input[index..].chars().next() {
        if escape {
            escape = false;
            index += ch.len_utf8();
            continue;
        }
        if ch == '\\' && quote == '"' {
            escape = true;
            index += ch.len_utf8();
            continue;
        }
        if ch == quote {
            return index + ch.len_utf8();
        }
        index += ch.len_utf8();
    }

    input.len()
}

pub(super) fn looks_numeric(token: &str) -> bool {
    let stripped = token.trim_matches(',');
    !stripped.is_empty()
        && stripped
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '_' | '-' | '+' | 'x' | 'o' | 'b'))
}

pub(super) fn styled_text(
    text: &str,
    color: ratatui::style::Color,
    modifier: ratatui::style::Modifier,
) -> Span<'static> {
    Span::styled(
        text.to_string(),
        Style::default().fg(color).add_modifier(modifier),
    )
}
