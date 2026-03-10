use crate::appearance;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::path::Path;

// These renderers are intentionally heuristic. They provide a readable backup
// when syntect has no syntax for a file, but they are not meant to be exact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FallbackSyntax {
    JsLike,
    Toml,
    Json,
    Yaml,
}

impl FallbackSyntax {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::JsLike => "TypeScript",
            Self::Toml => "TOML",
            Self::Json => "JSON",
            Self::Yaml => "YAML",
        }
    }

    pub(super) fn detail_label(self) -> String {
        format!("{} (best-effort)", self.label())
    }
}

pub(super) fn preview_fallback_syntax(path: &Path) -> Option<FallbackSyntax> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();

    match ext.as_str() {
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" => Some(FallbackSyntax::JsLike),
        "toml" => Some(FallbackSyntax::Toml),
        "json" | "jsonc" => Some(FallbackSyntax::Json),
        "yaml" | "yml" => Some(FallbackSyntax::Yaml),
        _ => match name.as_str() {
            "cargo.lock" | "poetry.lock" => Some(FallbackSyntax::Toml),
            "package.json" | "package-lock.json" | "tsconfig.json" | "deno.json" | "deno.jsonc" => {
                Some(FallbackSyntax::Json)
            }
            "compose.yml"
            | "compose.yaml"
            | "docker-compose.yml"
            | "docker-compose.yaml"
            | "pnpm-lock.yaml"
            | "pnpm-workspace.yaml" => Some(FallbackSyntax::Yaml),
            _ => None,
        },
    }
}

pub(super) fn render_fallback_code_preview(
    text: &str,
    syntax: FallbackSyntax,
    line_numbers: bool,
) -> Vec<Line<'static>> {
    let code_palette = appearance::code_preview_palette();
    let source_lines = super::collect_preview_lines(text);
    let number_width = super::line_number_width(source_lines.len());
    let mut rendered = Vec::new();

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
            FallbackSyntax::Toml => highlight_toml_line(line, code_palette),
            FallbackSyntax::Json => highlight_json_line(line, code_palette),
            FallbackSyntax::Yaml => highlight_yaml_line(line, code_palette),
        };
        spans.extend(body);
        rendered.push(Line::from(spans));
    }

    if rendered.is_empty() {
        rendered.push(Line::from("File is empty"));
    }
    rendered
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
    let bytes = input.as_bytes();
    let mut spans = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() {
        let ch = bytes[index] as char;
        if ch.is_whitespace() {
            let start = index;
            while index < bytes.len() && (bytes[index] as char).is_whitespace() {
                index += 1;
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
            spans.push(styled_text(
                &input[index..index + 1],
                palette.operator,
                Modifier::empty(),
            ));
            index += 1;
            continue;
        }

        let start = index;
        while index < bytes.len() {
            let current = bytes[index] as char;
            if current.is_whitespace()
                || "[]{}(),:#".contains(current)
                || current == '"'
                || current == '\''
            {
                break;
            }
            index += 1;
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

fn scan_quoted_segment(input: &str, start: usize) -> usize {
    let bytes = input.as_bytes();
    let quote = bytes[start] as char;
    let mut index = start + 1;
    let mut escape = false;

    while index < bytes.len() {
        let ch = bytes[index] as char;
        if escape {
            escape = false;
            index += 1;
            continue;
        }
        if ch == '\\' && quote == '"' {
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
    use std::path::Path;

    #[test]
    fn fallback_detail_label_marks_best_effort() {
        assert_eq!(FallbackSyntax::Json.detail_label(), "JSON (best-effort)");
    }

    #[test]
    fn jsonc_file_can_use_fallback_highlighting() {
        assert_eq!(
            preview_fallback_syntax(Path::new("deno.jsonc")),
            Some(FallbackSyntax::Json)
        );
    }
}
