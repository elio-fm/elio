use super::common::{looks_numeric, scan_string, styled_text};
use crate::appearance;
use ratatui::{style::Modifier, text::Span};

#[derive(Default)]
pub(super) struct ShellState {
    heredoc: Option<HeredocState>,
}

struct HeredocState {
    delimiter: String,
    strip_tabs: bool,
}

pub(super) fn highlight_shell_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
    state: &mut ShellState,
) -> Vec<Span<'static>> {
    if let Some(spans) = highlight_heredoc_line(line, palette, state) {
        return spans;
    }

    if let Some(rest) = line.strip_prefix("#!") {
        return vec![
            styled_text("#!", palette.r#macro, Modifier::BOLD),
            styled_text(rest, palette.string, Modifier::empty()),
        ];
    }

    let (body, comment) = split_shell_comment(line);
    let trimmed = body.trim_start();
    let indent = &body[..body.len().saturating_sub(trimmed.len())];
    let mut spans = vec![Span::raw(indent.to_string())];
    spans.extend(highlight_shell_fragment(trimmed, palette, true));

    if let Some(heredoc) = detect_heredoc_start(body) {
        state.heredoc = Some(heredoc);
    }

    if let Some(comment) = comment {
        if !body.is_empty() {
            spans.push(Span::raw(" ".to_string()));
        }
        spans.push(styled_text(comment, palette.comment, Modifier::ITALIC));
    }

    spans
}

fn highlight_heredoc_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
    state: &mut ShellState,
) -> Option<Vec<Span<'static>>> {
    let heredoc = state.heredoc.as_ref()?;
    let candidate = if heredoc.strip_tabs {
        line.trim_start_matches('\t')
    } else {
        line
    };

    if candidate == heredoc.delimiter {
        state.heredoc = None;
        return Some(vec![styled_text(line, palette.r#macro, Modifier::BOLD)]);
    }

    Some(vec![styled_text(line, palette.string, Modifier::empty())])
}

fn highlight_shell_fragment(
    input: &str,
    palette: appearance::CodePreviewPalette,
    mut expect_command: bool,
) -> Vec<Span<'static>> {
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
            let end = scan_shell_expansion(input, index);
            spans.push(styled_text(
                &input[index..end],
                palette.r#macro,
                Modifier::BOLD,
            ));
            index = end;
            expect_command = false;
            continue;
        }

        if ch == '$' {
            let end = scan_shell_variable(input, index);
            spans.push(styled_text(
                &input[index..end],
                palette.r#macro,
                Modifier::BOLD,
            ));
            index = end;
            expect_command = false;
            continue;
        }

        if matches!(ch, '"' | '\'' | '`') {
            let end = scan_string(input, index, ch);
            spans.push(styled_text(
                &input[index..end],
                palette.string,
                Modifier::empty(),
            ));
            index = end;
            expect_command = false;
            continue;
        }

        if is_shell_operator_start(input, index) {
            let end = consume_shell_operator(input, index);
            let operator = &input[index..end];
            spans.push(styled_text(operator, palette.operator, Modifier::empty()));
            index = end;
            expect_command = matches!(operator, "|" | "||" | "|&" | "&&" | ";" | ";;" | "(" | "{");
            continue;
        }

        let start = index;
        while let Some(current) = input[index..].chars().next() {
            if current.is_whitespace()
                || current == '$'
                || matches!(current, '"' | '\'' | '`')
                || is_shell_operator_char(current)
            {
                break;
            }
            index += current.len_utf8();
        }

        let token = &input[start..index];
        if let Some((name, value)) = split_shell_assignment_token(token) {
            spans.push(styled_text(name, palette.parameter, Modifier::BOLD));
            spans.push(styled_text("=", palette.operator, Modifier::empty()));
            if !value.is_empty() {
                spans.extend(highlight_shell_fragment(value, palette, false));
            }
            continue;
        }

        let next_significant = input[index..]
            .chars()
            .find(|current| !current.is_whitespace());
        let is_function_name = token.ends_with("()") || next_significant == Some('(');
        let (color, modifier) = if is_shell_keyword(token) {
            (palette.keyword, Modifier::BOLD)
        } else if is_shell_test_operator(token) {
            (palette.operator, Modifier::empty())
        } else if is_shell_builtin(token)
            || token == "["
            || token == "[["
            || token == "]]"
            || expect_command
            || is_function_name
        {
            (palette.function, Modifier::empty())
        } else if looks_numeric(token) {
            (palette.constant, Modifier::empty())
        } else {
            (palette.fg, Modifier::empty())
        };
        spans.push(styled_text(token, color, modifier));
        expect_command = token == "function";
    }

    spans
}

fn split_shell_comment(input: &str) -> (&str, Option<&str>) {
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
            if ch == '\\' && quote != '\'' {
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

        if matches!(ch, '"' | '\'' | '`') {
            quote = ch;
            index += ch.len_utf8();
            continue;
        }

        if input[index..].starts_with("$(") || input[index..].starts_with("${") {
            index = scan_shell_expansion(input, index);
            continue;
        }

        if ch == '#'
            && (index == 0
                || input[..index].chars().last().is_some_and(|previous| {
                    previous.is_whitespace() || matches!(previous, ';' | '|')
                }))
        {
            return (&input[..index], Some(&input[index..]));
        }

        index += ch.len_utf8();
    }

    (input, None)
}

fn detect_heredoc_start(input: &str) -> Option<HeredocState> {
    let mut index = 0usize;
    let mut quote = '\0';
    let mut escape = false;

    while index < input.len() {
        let ch = input[index..].chars().next().expect("valid utf-8 char");
        if quote != '\0' {
            if escape {
                escape = false;
                index += ch.len_utf8();
                continue;
            }
            if ch == '\\' && quote != '\'' {
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

        if matches!(ch, '"' | '\'' | '`') {
            quote = ch;
            index += ch.len_utf8();
            continue;
        }

        if input[index..].starts_with("<<-") || input[index..].starts_with("<<") {
            let strip_tabs = input[index..].starts_with("<<-");
            index += if strip_tabs { 3 } else { 2 };
            while let Some(current) = input[index..].chars().next() {
                if !current.is_whitespace() {
                    break;
                }
                index += current.len_utf8();
            }
            if index >= input.len() {
                return None;
            }
            let delimiter_end = scan_heredoc_delimiter(input, index);
            let raw_delimiter = &input[index..delimiter_end];
            let delimiter = normalize_heredoc_delimiter(raw_delimiter);
            if delimiter.is_empty() {
                return None;
            }
            return Some(HeredocState {
                delimiter,
                strip_tabs,
            });
        }

        index += ch.len_utf8();
    }

    None
}

fn scan_heredoc_delimiter(input: &str, start: usize) -> usize {
    let ch = input[start..].chars().next().unwrap_or(' ');
    if matches!(ch, '"' | '\'' | '`') {
        return scan_string(input, start, ch);
    }

    let mut index = start;
    while let Some(current) = input[index..].chars().next() {
        if current.is_whitespace() || matches!(current, ';' | '|' | '<' | '>' | ')' | '(') {
            break;
        }
        index += current.len_utf8();
    }
    index
}

fn normalize_heredoc_delimiter(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
            || (trimmed.starts_with('`') && trimmed.ends_with('`')))
    {
        return trimmed[1..trimmed.len() - 1].to_string();
    }

    trimmed.trim_start_matches('\\').to_string()
}

fn consume_shell_operator(input: &str, start: usize) -> usize {
    const THREE_CHAR: [&str; 2] = ["<<-", "<<<"];
    const TWO_CHAR: [&str; 13] = [
        "[[", "]]", "&&", "||", ";;", "<<", ">>", "|&", ">&", "&>", ">|", "<(", ">(",
    ];

    for token in THREE_CHAR {
        if input[start..].starts_with(token) {
            return start + token.len();
        }
    }
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

fn scan_shell_expansion(input: &str, start: usize) -> usize {
    let opener = input[start + 1..].chars().next().unwrap_or('(');
    let closer = if opener == '{' { '}' } else { ')' };
    let mut index = start + 2;
    let mut depth = 1usize;
    let mut quote = '\0';
    let mut escape = false;

    while index < input.len() {
        let ch = input[index..].chars().next().unwrap_or(closer);
        if quote != '\0' {
            if escape {
                escape = false;
                index += ch.len_utf8();
                continue;
            }
            if ch == '\\' && quote != '\'' {
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

        if matches!(ch, '"' | '\'' | '`') {
            quote = ch;
            index += ch.len_utf8();
            continue;
        }

        if ch == '$' && index + ch.len_utf8() < input.len() {
            let next = input[index + ch.len_utf8()..]
                .chars()
                .next()
                .unwrap_or(closer);
            if next == opener {
                depth += 1;
                index += ch.len_utf8() + next.len_utf8();
                continue;
            }
        }

        index += ch.len_utf8();
        if ch == closer {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return index;
            }
        }
    }

    input.len()
}

fn scan_shell_variable(input: &str, start: usize) -> usize {
    let mut index = start + 1;
    while let Some(ch) = input[index..].chars().next() {
        if ch.is_ascii_alphanumeric()
            || matches!(
                ch,
                '_' | '@' | '*' | '#' | '?' | '!' | '$' | '-' | '0'..='9'
            )
        {
            index += ch.len_utf8();
        } else {
            break;
        }
    }
    index.max(start + 1)
}

fn split_shell_assignment_token(token: &str) -> Option<(&str, &str)> {
    let index = token.find('=')?;
    let left = token[..index].trim_end();
    if left.is_empty()
        || !left
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        || left.chars().next().is_some_and(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    Some((left, &token[index + 1..]))
}

fn is_shell_operator_start(input: &str, index: usize) -> bool {
    input[index..].starts_with("[[")
        || input[index..].starts_with("]]")
        || input[index..].starts_with("&&")
        || input[index..].starts_with("||")
        || input[index..].starts_with(";;")
        || input[index..].starts_with("<<-")
        || input[index..].starts_with("<<<")
        || input[index..].starts_with("<<")
        || input[index..].starts_with(">>")
        || input[index..].starts_with("|&")
        || input[index..].starts_with(">&")
        || input[index..].starts_with("&>")
        || input[index..].starts_with(">|")
        || input[index..].starts_with("<(")
        || input[index..].starts_with(">(")
        || input[index..].starts_with('|')
        || input[index..].starts_with(';')
        || input[index..].starts_with('(')
        || input[index..].starts_with(')')
        || input[index..].starts_with('{')
        || input[index..].starts_with('}')
        || input[index..].starts_with('<')
        || input[index..].starts_with('>')
}

fn is_shell_operator_char(ch: char) -> bool {
    matches!(ch, '|' | '&' | ';' | '(' | ')' | '{' | '}' | '<' | '>')
}

fn is_shell_keyword(token: &str) -> bool {
    matches!(
        token,
        "if" | "then"
            | "elif"
            | "else"
            | "fi"
            | "for"
            | "while"
            | "until"
            | "do"
            | "done"
            | "case"
            | "in"
            | "esac"
            | "function"
            | "select"
            | "time"
            | "coproc"
            | "!"
    )
}

fn is_shell_builtin(token: &str) -> bool {
    matches!(
        token,
        "echo"
            | "printf"
            | "cd"
            | "export"
            | "local"
            | "readonly"
            | "unset"
            | "alias"
            | "test"
            | "exec"
            | "eval"
            | "source"
            | "."
            | "return"
            | "shift"
            | "trap"
            | "read"
            | "mapfile"
            | "command"
            | "type"
            | "typeset"
            | "declare"
            | "set"
            | "shopt"
            | "complete"
            | "builtin"
    )
}

fn is_shell_test_operator(token: &str) -> bool {
    matches!(
        token,
        "=" | "=="
            | "!="
            | "=~"
            | "-eq"
            | "-ne"
            | "-gt"
            | "-ge"
            | "-lt"
            | "-le"
            | "-a"
            | "-o"
            | "-n"
            | "-z"
            | "-f"
            | "-d"
            | "-e"
            | "-r"
            | "-w"
            | "-x"
            | "-s"
            | "-L"
            | "-h"
            | "-p"
            | "-S"
            | "-t"
    )
}
