use super::common::{
    find_unquoted_token, looks_numeric, scan_make_variable, scan_string, styled_text,
};
use crate::ui::theme;
use ratatui::{style::Modifier, text::Span};

pub(super) fn highlight_make_line(
    line: &str,
    palette: theme::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let trimmed = line.trim_start();
    let indent = &line[..line.len().saturating_sub(trimmed.len())];

    if trimmed.is_empty() {
        return vec![Span::raw(line.to_string())];
    }

    if trimmed.starts_with('#') {
        return vec![
            Span::raw(indent.to_string()),
            styled_text(trimmed, palette.comment, Modifier::ITALIC),
        ];
    }

    if line.starts_with('\t') {
        let recipe = trimmed;
        let mut spans = vec![styled_text("\t", palette.operator, Modifier::empty())];
        spans.extend(highlight_make_recipe(recipe, palette));
        return spans;
    }

    let (body, comment) = split_make_comment(trimmed);
    let mut spans = vec![Span::raw(indent.to_string())];
    if let Some((directive, rest)) = split_make_directive(body) {
        spans.push(styled_text(directive, palette.keyword, Modifier::BOLD));
        spans.extend(highlight_make_fragment(rest, palette));
    } else if let Some((left, operator, right)) = split_make_assignment(body) {
        spans.push(styled_text(
            left.trim_end(),
            palette.parameter,
            Modifier::BOLD,
        ));
        spans.push(styled_text(operator, palette.operator, Modifier::empty()));
        spans.extend(highlight_make_fragment(right, palette));
    } else if let Some((targets, operator, prerequisites)) = split_make_rule(body) {
        spans.extend(highlight_make_targets(targets, palette));
        spans.push(styled_text(operator, palette.operator, Modifier::empty()));
        spans.extend(highlight_make_fragment(prerequisites, palette));
    } else {
        spans.extend(highlight_make_fragment(body, palette));
    }

    if let Some(comment) = comment {
        if !body.is_empty() {
            spans.push(Span::raw(" ".to_string()));
        }
        spans.push(styled_text(comment, palette.comment, Modifier::ITALIC));
    }

    spans
}

fn highlight_make_recipe(recipe: &str, palette: theme::CodePreviewPalette) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut remaining = recipe;

    while let Some(prefix) = remaining
        .chars()
        .next()
        .filter(|ch| matches!(ch, '@' | '-' | '+'))
    {
        let len = prefix.len_utf8();
        spans.push(styled_text(
            &remaining[..len],
            palette.operator,
            Modifier::empty(),
        ));
        remaining = &remaining[len..];
    }

    let trimmed = remaining.trim_start();
    let indent = &remaining[..remaining.len().saturating_sub(trimmed.len())];
    if !indent.is_empty() {
        spans.push(Span::raw(indent.to_string()));
    }
    if trimmed.is_empty() {
        return spans;
    }

    let command_end = trimmed
        .char_indices()
        .find(|(_, ch)| ch.is_whitespace())
        .map(|(index, _)| index)
        .unwrap_or(trimmed.len());
    spans.push(styled_text(
        &trimmed[..command_end],
        palette.function,
        Modifier::empty(),
    ));
    spans.extend(highlight_make_fragment(&trimmed[command_end..], palette));
    spans
}

fn highlight_make_targets(targets: &str, palette: theme::CodePreviewPalette) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for token in targets.split_whitespace() {
        if !spans.is_empty() {
            spans.push(Span::raw(" ".to_string()));
        }
        let color = if token.starts_with('.') {
            palette.keyword
        } else {
            palette.function
        };
        let modifier = if token.starts_with('.') {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };
        spans.push(styled_text(token, color, modifier));
    }
    spans
}

fn highlight_make_fragment(input: &str, palette: theme::CodePreviewPalette) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut index = 0usize;

    while index < input.len() {
        let ch = input[index..].chars().next().unwrap_or(' ');
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

        if input[index..].starts_with("$(") || input[index..].starts_with("${") {
            let end = scan_make_variable(input, index);
            spans.push(styled_text(
                &input[index..end],
                palette.r#macro,
                Modifier::BOLD,
            ));
            index = end;
            continue;
        }

        if ch == '$' {
            let end = index
                + ch.len_utf8()
                + input[index + ch.len_utf8()..]
                    .chars()
                    .next()
                    .map(char::len_utf8)
                    .unwrap_or(0);
            spans.push(styled_text(
                &input[index..end],
                palette.r#macro,
                Modifier::BOLD,
            ));
            index = end;
            continue;
        }

        if matches!(ch, '"' | '\'') {
            let end = scan_string(input, index, ch);
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
            while let Some(current) = input[index..].chars().next() {
                if current.is_ascii_alphanumeric() || matches!(current, '.' | '_' | '/' | '-') {
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

        if ":=+?!=|(){}".contains(ch) || (ch == ':' && input[index..].starts_with("::")) {
            let end = if input[index..].starts_with("::") {
                index + 2
            } else {
                index + ch.len_utf8()
            };
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
                || current == '$'
                || current == '"'
                || current == '\''
                || ":=+?!=|(){}".contains(current)
            {
                break;
            }
            index += current.len_utf8();
        }
        let token = &input[start..index];
        let color = if token.starts_with('.') {
            palette.keyword
        } else if looks_numeric(token) {
            palette.constant
        } else {
            palette.fg
        };
        let modifier = if token.starts_with('.') {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };
        spans.push(styled_text(token, color, modifier));
    }

    spans
}

fn split_make_comment(input: &str) -> (&str, Option<&str>) {
    let mut quote = '\0';
    let mut escape = false;
    let mut index = 0usize;

    while index < input.len() {
        let ch = input[index..].chars().next().expect("valid utf-8 char");
        if quote != '\0' {
            if escape {
                escape = false;
                index += ch.len_utf8();
                continue;
            }
            if ch == '\\' {
                escape = true;
                index += ch.len_utf8();
                continue;
            }
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

        if ch == '#' {
            return (&input[..index], Some(&input[index..]));
        }

        index += ch.len_utf8();
    }

    (input, None)
}

fn split_make_assignment(input: &str) -> Option<(&str, &str, &str)> {
    for operator in ["::=", ":=", "+=", "?=", "!=", "="] {
        if let Some(index) = find_unquoted_token(input, operator) {
            return Some((&input[..index], operator, &input[index + operator.len()..]));
        }
    }
    None
}

fn split_make_rule(input: &str) -> Option<(&str, &str, &str)> {
    let index = find_unquoted_token(input, ":")?;
    if input[index..].starts_with(":=") || input[index..].starts_with("::=") {
        return None;
    }
    let operator = if input[index..].starts_with("::") {
        "::"
    } else {
        ":"
    };
    Some((&input[..index], operator, &input[index + operator.len()..]))
}

fn split_make_directive(input: &str) -> Option<(&str, &str)> {
    const DIRECTIVES: [&str; 15] = [
        "ifeq", "ifneq", "ifdef", "ifndef", "else", "endif", "include", "-include", "sinclude",
        "override", "export", "unexport", "define", "endef", "vpath",
    ];

    let trimmed = input.trim_start();
    let offset = input.len().saturating_sub(trimmed.len());
    for directive in DIRECTIVES {
        if trimmed == directive {
            return Some((&input[offset..], ""));
        }
        if let Some(rest) = trimmed.strip_prefix(directive)
            && rest.chars().next().is_some_and(char::is_whitespace)
        {
            let consumed = directive.len();
            return Some((&input[offset..offset + consumed], &trimmed[consumed..]));
        }
    }
    None
}
