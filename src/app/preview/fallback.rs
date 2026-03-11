use crate::{appearance, file_facts::FallbackSyntax};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub(super) fn render_fallback_code_preview(
    text: &str,
    syntax: FallbackSyntax,
    line_numbers: bool,
) -> Vec<Line<'static>> {
    let code_palette = appearance::code_preview_palette();
    let source_lines = super::collect_preview_lines(text);
    let number_width = super::line_number_width(source_lines.len());
    let mut rendered = Vec::new();
    let mut jsonc_block_comment = false;
    let mut markup_block_comment = false;
    let mut css_block_comment = false;

    for (index, line) in source_lines.iter().enumerate() {
        let mut spans = Vec::new();
        if line_numbers {
            spans.push(super::line_number_span(index + 1, number_width));
        } else {
            spans.push(Span::styled(
                "│ ",
                Style::default().fg(code_palette.line_number),
            ));
        }

        let body = match syntax {
            FallbackSyntax::JsLike => highlight_js_like_line(line, code_palette),
            FallbackSyntax::Markup => {
                highlight_markup_line(line, code_palette, &mut markup_block_comment)
            }
            FallbackSyntax::Css => highlight_css_line(line, code_palette, &mut css_block_comment),
            FallbackSyntax::Toml => highlight_toml_line(line, code_palette),
            FallbackSyntax::Json => highlight_json_line(line, code_palette),
            FallbackSyntax::Jsonc => {
                highlight_jsonc_line(line, code_palette, &mut jsonc_block_comment)
            }
            FallbackSyntax::Yaml => highlight_yaml_line(line, code_palette),
            FallbackSyntax::Log => highlight_log_line(line, code_palette),
            FallbackSyntax::Ini | FallbackSyntax::DesktopEntry => {
                highlight_ini_line(line, code_palette, syntax == FallbackSyntax::DesktopEntry)
            }
        };
        spans.extend(body);
        rendered.push(Line::from(spans));
    }

    if rendered.is_empty() {
        rendered.push(Line::from("File is empty"));
    }
    rendered
}

fn highlight_ini_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
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

fn highlight_log_line(line: &str, palette: appearance::CodePreviewPalette) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let trimmed = line.trim_start();
    let indent = &line[..line.len().saturating_sub(trimmed.len())];
    spans.push(Span::raw(indent.to_string()));

    let mut rest = trimmed;
    if let Some((timestamp, remaining)) = split_log_timestamp(rest) {
        spans.push(styled_text(timestamp, palette.comment, Modifier::empty()));
        rest = remaining;
        if let Some((whitespace, remaining)) = split_leading_whitespace(rest) {
            spans.push(Span::raw(whitespace.to_string()));
            rest = remaining;
        }
    }

    if let Some((level, remaining)) = split_log_level(rest) {
        spans.push(styled_text(
            level,
            log_level_color(level, palette),
            Modifier::BOLD,
        ));
        rest = remaining;
        if let Some(space_end) = rest
            .char_indices()
            .find(|(_, ch)| !ch.is_whitespace())
            .map(|(index, _)| index)
        {
            spans.push(Span::raw(rest[..space_end].to_string()));
            rest = &rest[space_end..];
        } else {
            spans.push(Span::raw(rest.to_string()));
            return spans;
        }
    }

    spans.extend(highlight_log_message(rest, palette));
    spans
}

fn highlight_log_message(
    line: &str,
    palette: appearance::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current = String::new();

    for token in line.split_inclusive(char::is_whitespace) {
        let word = token.trim_end_matches(char::is_whitespace);
        let suffix = &token[word.len()..];

        if word.is_empty() {
            current.push_str(token);
            continue;
        }

        let styled = if let Some((left, right)) = split_unquoted_once(word, '=') {
            if !current.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut current)));
            }
            spans.push(styled_text(left, palette.parameter, Modifier::BOLD));
            spans.push(styled_text("=", palette.operator, Modifier::empty()));
            spans.extend(highlight_value_fragment(right, palette));
            if !suffix.is_empty() {
                spans.push(Span::raw(suffix.to_string()));
            }
            continue;
        } else if looks_numeric(word.trim_matches(['[', ']', '(', ')', ',', ';'])) {
            Some(styled_text(word, palette.constant, Modifier::empty()))
        } else if word.starts_with('[') && word.ends_with(']') {
            Some(styled_text(word, palette.r#type, Modifier::empty()))
        } else if word.ends_with(':') && word.len() > 1 {
            Some(styled_text(word, palette.function, Modifier::empty()))
        } else {
            None
        };

        if let Some(span) = styled {
            if !current.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut current)));
            }
            spans.push(span);
            if !suffix.is_empty() {
                spans.push(Span::raw(suffix.to_string()));
            }
        } else {
            current.push_str(token);
        }
    }

    if !current.is_empty() {
        spans.push(Span::raw(current));
    }

    spans
}

fn split_log_timestamp(input: &str) -> Option<(&str, &str)> {
    let mut end = 0usize;
    let mut separators = 0usize;

    for (index, ch) in input.char_indices() {
        if ch.is_ascii_digit() || matches!(ch, '-' | ':' | 'T' | 'Z' | '.' | '+' | '/' | ',') {
            end = index + ch.len_utf8();
            if matches!(ch, '-' | ':' | 'T' | '/') {
                separators += 1;
            }
            continue;
        }
        break;
    }

    if end == 0 || separators < 2 {
        return None;
    }

    Some((&input[..end], &input[end..]))
}

fn split_leading_whitespace(input: &str) -> Option<(&str, &str)> {
    let end = input
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    if end == 0 {
        None
    } else {
        Some((&input[..end], &input[end..]))
    }
}

fn split_log_level(input: &str) -> Option<(&str, &str)> {
    let trimmed = input.trim_start();
    let offset = input.len().saturating_sub(trimmed.len());
    let mut chars = trimmed.char_indices();
    let (start, first) = chars.next()?;

    let (level, consumed) = if first == '[' {
        let end = trimmed.find(']')?;
        (&trimmed[start..=end], end + 1)
    } else {
        let end = trimmed
            .char_indices()
            .find(|(_, ch)| ch.is_whitespace() || matches!(ch, ':' | ',' | ';'))
            .map(|(index, _)| index)
            .unwrap_or(trimmed.len());
        (&trimmed[..end], end)
    };

    let normalized = level
        .trim_matches(|ch| matches!(ch, '[' | ']'))
        .to_ascii_uppercase();
    if !matches!(
        normalized.as_str(),
        "TRACE" | "DEBUG" | "INFO" | "NOTICE" | "WARN" | "WARNING" | "ERROR" | "ERR" | "FATAL"
    ) {
        return None;
    }

    Some((
        &input[offset..offset + consumed],
        &input[offset + consumed..],
    ))
}

fn log_level_color(level: &str, palette: appearance::CodePreviewPalette) -> Color {
    match level
        .trim_matches(|ch| matches!(ch, '[' | ']'))
        .to_ascii_uppercase()
        .as_str()
    {
        "TRACE" => palette.comment,
        "DEBUG" => palette.constant,
        "INFO" | "NOTICE" => palette.function,
        "WARN" | "WARNING" => palette.keyword,
        "ERROR" | "ERR" | "FATAL" => palette.invalid,
        _ => palette.fg,
    }
}

fn highlight_markup_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
    in_block_comment: &mut bool,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    for (is_comment, segment) in split_markup_segments(line, in_block_comment) {
        if is_comment {
            spans.push(styled_text(segment, palette.comment, Modifier::ITALIC));
        } else {
            spans.extend(highlight_markup_segment(segment, palette));
        }
    }

    spans
}

fn highlight_markup_segment(
    input: &str,
    palette: appearance::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut index = 0usize;

    while index < input.len() {
        let Some(offset) = input[index..].find('<') else {
            spans.push(Span::raw(input[index..].to_string()));
            break;
        };
        let tag_start = index + offset;
        if tag_start > index {
            spans.push(Span::raw(input[index..tag_start].to_string()));
        }

        let tag_end = scan_markup_tag_end(input, tag_start);
        spans.extend(highlight_markup_tag(&input[tag_start..tag_end], palette));
        index = tag_end;
    }

    if spans.is_empty() {
        spans.push(Span::raw(String::new()));
    }
    spans
}

fn highlight_markup_tag(tag: &str, palette: appearance::CodePreviewPalette) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut index = 0usize;

    let opener = if tag.starts_with("</") {
        "</"
    } else if tag.starts_with("<?") {
        "<?"
    } else if tag.starts_with("<!") {
        "<!"
    } else {
        "<"
    };
    spans.push(styled_text(opener, palette.operator, Modifier::empty()));
    index += opener.len();

    let name_start = index;
    while index < tag.len() {
        let ch = tag[index..].chars().next().unwrap_or('>');
        if ch.is_whitespace() || matches!(ch, '/' | '>' | '?' | '=') {
            break;
        }
        index += ch.len_utf8();
    }
    if index > name_start {
        let name = &tag[name_start..index];
        let color = if opener == "<!" {
            palette.keyword
        } else {
            palette.tag
        };
        let modifier = if opener == "<!" {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };
        spans.push(styled_text(name, color, modifier));
    }

    while index < tag.len() {
        let ch = tag[index..].chars().next().unwrap_or('>');
        if ch.is_whitespace() {
            let start = index;
            while index < tag.len() {
                let current = tag[index..].chars().next().unwrap_or('>');
                if !current.is_whitespace() {
                    break;
                }
                index += current.len_utf8();
            }
            spans.push(Span::raw(tag[start..index].to_string()));
            continue;
        }

        if tag[index..].starts_with("/>") || tag[index..].starts_with("?>") {
            spans.push(styled_text(
                &tag[index..index + 2],
                palette.operator,
                Modifier::empty(),
            ));
            index += 2;
            continue;
        }

        if matches!(ch, '>' | '/' | '?' | '=') {
            let end = index + ch.len_utf8();
            spans.push(styled_text(
                &tag[index..end],
                palette.operator,
                Modifier::empty(),
            ));
            index = end;
            continue;
        }

        if matches!(ch, '"' | '\'') {
            let end = scan_quoted_segment(tag, index);
            spans.push(styled_text(
                &tag[index..end],
                palette.string,
                Modifier::empty(),
            ));
            index = end;
            continue;
        }

        let start = index;
        while index < tag.len() {
            let current = tag[index..].chars().next().unwrap_or('>');
            if current.is_whitespace() || matches!(current, '/' | '>' | '?' | '=' | '"' | '\'') {
                break;
            }
            index += current.len_utf8();
        }
        spans.push(styled_text(
            &tag[start..index],
            palette.parameter,
            Modifier::BOLD,
        ));
    }

    spans
}

fn scan_markup_tag_end(input: &str, start: usize) -> usize {
    let mut index = start + 1;
    let mut quote = '\0';

    while let Some(ch) = input[index..].chars().next() {
        if quote != '\0' {
            if ch == quote {
                quote = '\0';
            }
            index += ch.len_utf8();
            continue;
        }

        if matches!(ch, '"' | '\'') {
            quote = ch;
            index += ch.len_utf8();
            continue;
        }

        index += ch.len_utf8();
        if ch == '>' {
            return index;
        }
    }

    input.len()
}

fn highlight_css_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
    in_block_comment: &mut bool,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    for (is_comment, segment) in split_block_comment_segments(line, in_block_comment, "/*", "*/") {
        if is_comment {
            spans.push(styled_text(segment, palette.comment, Modifier::ITALIC));
        } else {
            spans.extend(highlight_css_segment(segment, palette));
        }
    }

    spans
}

fn highlight_css_segment(
    input: &str,
    palette: appearance::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut index = 0usize;

    while index < input.len() {
        let ch = input[index..].chars().next().unwrap_or(' ');
        if ch.is_whitespace() {
            let start = index;
            while index < input.len() {
                let current = input[index..].chars().next().unwrap_or(' ');
                if !current.is_whitespace() {
                    break;
                }
                index += current.len_utf8();
            }
            spans.push(Span::raw(input[start..index].to_string()));
            continue;
        }

        if matches!(ch, '"' | '\'') {
            let end = scan_quoted_segment(input, index);
            spans.push(styled_text(
                &input[index..end],
                palette.string,
                Modifier::empty(),
            ));
            index = end;
            continue;
        }

        if ch.is_ascii_digit() {
            let start = index;
            index += ch.len_utf8();
            while index < input.len() {
                let current = input[index..].chars().next().unwrap_or(' ');
                if current.is_ascii_alphanumeric() || matches!(current, '.' | '%' | '-' | '_') {
                    index += current.len_utf8();
                } else {
                    break;
                }
            }
            spans.push(styled_text(
                &input[start..index],
                palette.constant,
                Modifier::empty(),
            ));
            continue;
        }

        if "{}[]():;,@".contains(ch) {
            let end = index + ch.len_utf8();
            let color = if ch == '@' {
                palette.keyword
            } else {
                palette.operator
            };
            let modifier = if ch == '@' {
                Modifier::BOLD
            } else {
                Modifier::empty()
            };
            spans.push(styled_text(&input[index..end], color, modifier));
            index = end;
            continue;
        }

        let start = index;
        while index < input.len() {
            let current = input[index..].chars().next().unwrap_or(' ');
            if current.is_whitespace() || "{}[]():;,@\"'".contains(current) {
                break;
            }
            index += current.len_utf8();
        }
        let token = &input[start..index];
        let color = if token.starts_with('@') {
            palette.keyword
        } else if next_non_whitespace_char(input, index) == Some(':') {
            palette.parameter
        } else if next_non_whitespace_char(input, index) == Some('(') {
            palette.function
        } else if token.starts_with('.') || token.starts_with('#') {
            palette.tag
        } else {
            palette.fg
        };
        let modifier = if color == palette.parameter || color == palette.keyword {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };
        spans.push(styled_text(token, color, modifier));
    }

    spans
}

fn highlight_js_like_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let (body, comment) = split_line_comment(line);
    let bytes = body.as_bytes();
    let mut spans = Vec::new();
    let mut index = 0usize;
    let mut last_word: Option<String> = None;

    while index < bytes.len() {
        let ch = bytes[index] as char;
        if ch.is_whitespace() {
            let start = index;
            while index < bytes.len() && (bytes[index] as char).is_whitespace() {
                index += 1;
            }
            spans.push(Span::raw(body[start..index].to_string()));
            continue;
        }

        if matches!(ch, '"' | '\'' | '`') {
            let end = scan_string(body, index, ch);
            spans.push(styled_text(
                &body[index..end],
                palette.string,
                Modifier::empty(),
            ));
            index = end;
            last_word = None;
            continue;
        }

        if ch.is_ascii_alphabetic() || ch == '_' || ch == '$' {
            let start = index;
            index += 1;
            while index < bytes.len() {
                let current = bytes[index] as char;
                if current.is_ascii_alphanumeric() || current == '_' || current == '$' {
                    index += 1;
                } else {
                    break;
                }
            }
            let token = &body[start..index];
            let next = next_non_whitespace_char(body, index);
            let color = if is_js_keyword(token) {
                palette.keyword
            } else if matches!(
                last_word.as_deref(),
                Some("function")
                    | Some("class")
                    | Some("interface")
                    | Some("type")
                    | Some("enum")
                    | Some("namespace")
            ) {
                if matches!(
                    last_word.as_deref(),
                    Some("class" | "interface" | "type" | "enum" | "namespace")
                ) {
                    palette.r#type
                } else {
                    palette.function
                }
            } else if next == Some('(') {
                palette.function
            } else if token
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_uppercase())
            {
                palette.r#type
            } else {
                palette.fg
            };
            spans.push(styled_text(token, color, Modifier::empty()));
            last_word = Some(token.to_string());
            continue;
        }

        if ch.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < bytes.len() {
                let current = bytes[index] as char;
                if current.is_ascii_alphanumeric() || matches!(current, '.' | '_') {
                    index += 1;
                } else {
                    break;
                }
            }
            spans.push(styled_text(
                &body[start..index],
                palette.constant,
                Modifier::empty(),
            ));
            last_word = None;
            continue;
        }

        let end = consume_operator(body, index);
        let token = &body[index..end];
        let color = if token == "=>"
            || token == "::"
            || token == "?."
            || token == "??"
            || token == "&&"
            || token == "||"
        {
            palette.operator
        } else if token == "<" || token == ">" || token == "</" || token == "/>" {
            palette.tag
        } else {
            palette.operator
        };
        spans.push(styled_text(token, color, Modifier::empty()));
        index = end;
        last_word = None;
    }

    if let Some(comment) = comment {
        if !body.is_empty() {
            spans.push(Span::raw(" ".to_string()));
        }
        spans.push(styled_text(comment, palette.comment, Modifier::ITALIC));
    }

    spans
}

fn highlight_toml_line(line: &str, palette: appearance::CodePreviewPalette) -> Vec<Span<'static>> {
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

fn highlight_json_line(line: &str, palette: appearance::CodePreviewPalette) -> Vec<Span<'static>> {
    let bytes = line.as_bytes();
    let mut spans = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() {
        let ch = bytes[index] as char;
        if ch.is_whitespace() {
            let start = index;
            while index < bytes.len() && (bytes[index] as char).is_whitespace() {
                index += 1;
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
                &line[index..index + 1],
                palette.operator,
                Modifier::empty(),
            ));
            index += 1;
            continue;
        }

        let start = index;
        while index < bytes.len() {
            let current = bytes[index] as char;
            if current.is_whitespace() || "{}[]:,".contains(current) {
                break;
            }
            index += 1;
        }
        spans.extend(highlight_scalar_token(&line[start..index], palette));
    }

    spans
}

fn highlight_jsonc_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
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

fn highlight_yaml_line(line: &str, palette: appearance::CodePreviewPalette) -> Vec<Span<'static>> {
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

fn highlight_value_fragment(
    value: &str,
    palette: appearance::CodePreviewPalette,
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

fn highlight_token_stream(
    input: &str,
    palette: appearance::CodePreviewPalette,
) -> Vec<Span<'static>> {
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

fn highlight_scalar_token(
    token: &str,
    palette: appearance::CodePreviewPalette,
) -> Vec<Span<'static>> {
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

fn split_comment(input: &str) -> (&str, Option<&str>) {
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

fn split_unquoted_once(input: &str, needle: char) -> Option<(&str, &str)> {
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

fn split_jsonc_segments<'a>(line: &'a str, in_block_comment: &mut bool) -> Vec<(bool, &'a str)> {
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

fn split_markup_segments<'a>(line: &'a str, in_block_comment: &mut bool) -> Vec<(bool, &'a str)> {
    split_block_comment_segments(line, in_block_comment, "<!--", "-->")
}

fn split_block_comment_segments<'a>(
    line: &'a str,
    in_block_comment: &mut bool,
    start_delim: &str,
    end_delim: &str,
) -> Vec<(bool, &'a str)> {
    let mut segments = Vec::new();
    let mut cursor = 0usize;

    while cursor < line.len() {
        if *in_block_comment {
            let comment_start = cursor;
            if let Some(offset) = line[cursor..].find(end_delim) {
                let end = cursor + offset + end_delim.len();
                segments.push((true, &line[comment_start..end]));
                *in_block_comment = false;
                cursor = end;
            } else {
                segments.push((true, &line[comment_start..]));
                return segments;
            }
            continue;
        }

        if let Some(offset) = line[cursor..].find(start_delim) {
            let start = cursor + offset;
            if start > cursor {
                segments.push((false, &line[cursor..start]));
            }

            let search_start = start + start_delim.len();
            if let Some(close_offset) = line[search_start..].find(end_delim) {
                let end = search_start + close_offset + end_delim.len();
                segments.push((true, &line[start..end]));
                cursor = end;
            } else {
                segments.push((true, &line[start..]));
                *in_block_comment = true;
                return segments;
            }
        } else {
            segments.push((false, &line[cursor..]));
            return segments;
        }
    }

    if segments.is_empty() {
        segments.push((false, line));
    }
    segments
}

fn scan_quoted_segment(input: &str, start: usize) -> usize {
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

fn scan_string(input: &str, start: usize, quote: char) -> usize {
    let bytes = input.as_bytes();
    let mut index = start + 1;
    let mut escape = false;

    while index < bytes.len() {
        let ch = bytes[index] as char;
        if escape {
            escape = false;
            index += 1;
            continue;
        }
        if ch == '\\' {
            escape = true;
            index += 1;
            continue;
        }
        if ch == quote {
            return index + 1;
        }
        index += 1;
    }

    input.len()
}

fn split_line_comment(input: &str) -> (&str, Option<&str>) {
    let bytes = input.as_bytes();
    let mut index = 0usize;
    let mut quote = '\0';
    let mut escape = false;

    while index < bytes.len() {
        let ch = bytes[index] as char;
        if quote != '\0' {
            if escape {
                escape = false;
                index += 1;
                continue;
            }
            if ch == '\\' {
                escape = true;
                index += 1;
                continue;
            }
            if ch == quote {
                quote = '\0';
            }
            index += 1;
            continue;
        }

        if matches!(ch, '"' | '\'' | '`') {
            quote = ch;
            index += 1;
            continue;
        }

        if ch == '/'
            && bytes
                .get(index + 1)
                .is_some_and(|next| *next as char == '/')
        {
            return (&input[..index], Some(&input[index..]));
        }

        index += 1;
    }

    (input, None)
}

fn next_non_whitespace_char(input: &str, start: usize) -> Option<char> {
    input[start..].chars().find(|ch| !ch.is_whitespace())
}

fn consume_operator(input: &str, start: usize) -> usize {
    const TWO_CHAR: [&str; 12] = [
        "=>", "::", "?.", "??", "&&", "||", "==", "!=", "<=", ">=", "</", "/>",
    ];
    for token in TWO_CHAR {
        if input[start..].starts_with(token) {
            return start + token.len();
        }
    }
    start
        + input[start..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(1)
}

fn is_js_keyword(token: &str) -> bool {
    matches!(
        token,
        "const"
            | "let"
            | "var"
            | "function"
            | "return"
            | "export"
            | "import"
            | "from"
            | "default"
            | "if"
            | "else"
            | "switch"
            | "case"
            | "break"
            | "continue"
            | "for"
            | "while"
            | "do"
            | "try"
            | "catch"
            | "finally"
            | "throw"
            | "new"
            | "class"
            | "extends"
            | "async"
            | "await"
            | "typeof"
            | "instanceof"
            | "in"
            | "of"
            | "this"
            | "super"
            | "interface"
            | "type"
            | "enum"
            | "implements"
            | "namespace"
            | "public"
            | "private"
            | "protected"
            | "readonly"
            | "as"
            | "declare"
            | "satisfies"
            | "infer"
            | "keyof"
    )
}

fn looks_numeric(token: &str) -> bool {
    let stripped = token.trim_matches(',');
    !stripped.is_empty()
        && stripped
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '_' | '-' | '+' | 'x' | 'o' | 'b'))
}

fn styled_text(text: &str, color: Color, modifier: Modifier) -> Span<'static> {
    Span::styled(
        text.to_string(),
        Style::default().fg(color).add_modifier(modifier),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_detail_label_marks_best_effort() {
        assert_eq!(FallbackSyntax::Json.detail_label(), "JSON (best-effort)");
    }

    #[test]
    fn markup_detail_label_marks_best_effort() {
        assert_eq!(
            FallbackSyntax::Markup.detail_label(),
            "Markup (best-effort)"
        );
    }

    #[test]
    fn jsonc_fallback_renderer_keeps_comments() {
        let lines = render_fallback_code_preview(
            "{\n  // comment\n  \"name\": \"elio\"\n}\n",
            FallbackSyntax::Jsonc,
            true,
        );

        assert!(
            lines[1]
                .spans
                .iter()
                .any(|span| span.content.contains("// comment"))
        );
    }

    #[test]
    fn jsonc_fallback_renderer_keeps_multiline_block_comments() {
        let lines = render_fallback_code_preview(
            "{\n  /* first line\n     second line */\n  \"name\": \"elio\"\n}\n",
            FallbackSyntax::Jsonc,
            true,
        );

        assert!(
            lines[1]
                .spans
                .iter()
                .any(|span| span.content.contains("/* first line"))
        );
        assert!(
            lines[2]
                .spans
                .iter()
                .any(|span| span.content.contains("second line */"))
        );
    }

    #[test]
    fn jsonc_detail_label_marks_best_effort() {
        assert_eq!(FallbackSyntax::Jsonc.detail_label(), "JSONC (best-effort)");
    }

    #[test]
    fn desktop_fallback_renderer_handles_unicode_values() {
        let lines = render_fallback_code_preview(
            "[Desktop Entry]\nName=エリオ\nName[ja]=日本語アプリ\n",
            FallbackSyntax::DesktopEntry,
            true,
        );

        assert!(
            lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("日本語アプリ"))
        );
    }

    #[test]
    fn log_fallback_renderer_highlights_levels_and_fields() {
        let lines = render_fallback_code_preview(
            "2026-03-10T12:00:00Z ERROR request_id=42 path=/login failed\n",
            FallbackSyntax::Log,
            true,
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

    #[test]
    fn markup_fallback_renderer_highlights_tags_attributes_and_comments() {
        let lines = render_fallback_code_preview(
            "<!-- note -->\n<div class=\"app\" data-id=\"42\">elio</div>\n",
            FallbackSyntax::Markup,
            true,
        );

        assert!(
            lines[0]
                .spans
                .iter()
                .any(|span| span.content.contains("<!-- note -->"))
        );
        assert!(
            lines[1]
                .spans
                .iter()
                .any(|span| span.content.contains("div"))
        );
        assert!(
            lines[1]
                .spans
                .iter()
                .any(|span| span.content.contains("class"))
        );
        assert!(
            lines[1]
                .spans
                .iter()
                .any(|span| span.content.contains("\"app\""))
        );
    }

    #[test]
    fn markup_fallback_renderer_keeps_multiline_comments() {
        let lines = render_fallback_code_preview(
            "<!-- first line\nsecond line -->\n<section />\n",
            FallbackSyntax::Markup,
            true,
        );

        assert!(
            lines[0]
                .spans
                .iter()
                .any(|span| span.content.contains("<!-- first line"))
        );
        assert!(
            lines[1]
                .spans
                .iter()
                .any(|span| span.content.contains("second line -->"))
        );
    }

    #[test]
    fn css_fallback_renderer_highlights_properties_and_values() {
        let lines = render_fallback_code_preview(
            ".app {\n  color: #fff;\n  margin: 12px;\n}\n",
            FallbackSyntax::Css,
            true,
        );

        assert!(
            lines[1]
                .spans
                .iter()
                .any(|span| span.content.contains("color"))
        );
        assert!(
            lines[2]
                .spans
                .iter()
                .any(|span| span.content.contains("12px"))
        );
    }
}
