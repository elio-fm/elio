use super::semantics::{SemanticRole, looks_like_shell_command_name, role_color};
use crate::preview::appearance::{self, CodePalette};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

#[derive(Clone, Copy)]
struct ShellLineState {
    command_position: bool,
    expect_heredoc_marker: bool,
    at_line_start: bool,
}

pub(super) fn render_shell_code_preview<F>(
    text: &str,
    line_numbers: bool,
    line_limit: usize,
    canceled: &F,
) -> Vec<Line<'static>>
where
    F: Fn() -> bool,
{
    let source_lines = crate::preview::collect_preview_lines_with_limit(
        text,
        crate::preview::clamp_code_preview_line_limit(line_limit),
    );
    let number_width = crate::preview::line_number_width(source_lines.len());
    let code_palette = appearance::code_palette();
    let mut rendered = Vec::new();

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

        spans.extend(render_shell_line(line, code_palette));
        rendered.push(Line::from(spans));
    }

    if rendered.is_empty() && !canceled() {
        rendered.push(Line::from("File is empty"));
    }

    rendered
}

pub(super) fn is_shell_like_syntax(code_syntax: &str) -> bool {
    matches!(code_syntax, "sh" | "bash" | "zsh" | "ksh")
}

fn render_shell_line(line: &str, palette: CodePalette) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut state = ShellLineState {
        command_position: true,
        expect_heredoc_marker: false,
        at_line_start: true,
    };
    let mut index = 0;

    while index < line.len() {
        let rest = &line[index..];

        if let Some(len) = consume_while(rest, |ch| ch.is_ascii_whitespace()) {
            spans.push(shell_span(&rest[..len], SemanticRole::Fg, palette));
            index += len;
            continue;
        }

        if state.at_line_start && rest.starts_with("#!") {
            spans.push(shell_span(rest, SemanticRole::Macro, palette));
            break;
        }

        if rest.starts_with('#') {
            spans.push(shell_span(rest, SemanticRole::Comment, palette));
            break;
        }

        if let Some(len) = consume_shell_double_quoted_string(rest, &mut spans, palette) {
            index += len;
            state.command_position = false;
            state.at_line_start = false;
            continue;
        }

        if let Some(len) = consume_shell_single_quoted_string(rest, &mut spans, palette) {
            index += len;
            state.command_position = false;
            state.at_line_start = false;
            continue;
        }

        if let Some((len, role, next_command_position, expect_heredoc_marker)) =
            match_shell_punctuation(rest)
        {
            spans.push(shell_span(&rest[..len], role, palette));
            index += len;
            state.command_position = next_command_position;
            state.expect_heredoc_marker = expect_heredoc_marker;
            state.at_line_start = false;
            continue;
        }

        if state.command_position
            && let Some((name_len, total_len)) = match_shell_assignment(rest)
        {
            spans.push(shell_span(
                &rest[..name_len],
                SemanticRole::Parameter,
                palette,
            ));
            spans.push(shell_span(
                &rest[name_len..total_len],
                SemanticRole::Operator,
                palette,
            ));
            index += total_len;
            state.command_position = true;
            state.at_line_start = false;
            continue;
        }

        if let Some(len) = consume_shell_variable(rest, &mut spans, palette) {
            index += len;
            state.command_position = false;
            state.at_line_start = false;
            continue;
        }

        let word_len =
            take_shell_word(rest).unwrap_or_else(|| rest.chars().next().unwrap().len_utf8());
        let word = &rest[..word_len];
        let role = classify_shell_word(word, &state);
        spans.push(shell_span(word, role, palette));
        state = advance_shell_state(state, word, role);
        index += word_len;
    }

    spans
}

fn shell_span(text: &str, role: SemanticRole, palette: CodePalette) -> Span<'static> {
    let mut rendered_style = Style::default().fg(role_color(role, palette));
    if role == SemanticRole::Invalid {
        rendered_style = rendered_style.add_modifier(Modifier::UNDERLINED);
    }

    Span::styled(crate::preview::expand_tabs(text), rendered_style)
}

fn consume_while(text: &str, predicate: impl Fn(char) -> bool) -> Option<usize> {
    let mut len = 0;
    for ch in text.chars() {
        if !predicate(ch) {
            break;
        }
        len += ch.len_utf8();
    }
    (len > 0).then_some(len)
}

fn consume_shell_single_quoted_string(
    text: &str,
    spans: &mut Vec<Span<'static>>,
    palette: CodePalette,
) -> Option<usize> {
    if !text.starts_with('\'') {
        return None;
    }

    let mut end = 1;
    while end < text.len() {
        let ch = text[end..].chars().next().unwrap();
        end += ch.len_utf8();
        if ch == '\'' {
            break;
        }
    }

    spans.push(shell_span(&text[..end], SemanticRole::String, palette));
    Some(end)
}

fn consume_shell_double_quoted_string(
    text: &str,
    spans: &mut Vec<Span<'static>>,
    palette: CodePalette,
) -> Option<usize> {
    if !text.starts_with('"') {
        return None;
    }

    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        if let Some(len) = consume_shell_variable(rest, spans, palette) {
            index += len;
            continue;
        }

        let ch = rest.chars().next().unwrap();
        let ch_len = ch.len_utf8();
        let mut end = index + ch_len;
        if ch == '"' && index != 0 {
            spans.push(shell_span(&text[index..end], SemanticRole::String, palette));
            return Some(end);
        }

        while end < text.len() {
            let next = text[end..].chars().next().unwrap();
            if next == '"' || next == '$' {
                break;
            }
            end += next.len_utf8();
        }

        spans.push(shell_span(&text[index..end], SemanticRole::String, palette));
        index = end;
    }

    Some(text.len())
}

fn consume_shell_variable(
    text: &str,
    spans: &mut Vec<Span<'static>>,
    palette: CodePalette,
) -> Option<usize> {
    if !text.starts_with('$') {
        return None;
    }

    let len = if text.starts_with("${") {
        consume_balanced(text, '{', '}')
    } else if text.starts_with("$(") {
        consume_balanced(text, '(', ')')
    } else {
        consume_simple_shell_variable(text)
    }?;

    spans.push(shell_span(&text[..len], SemanticRole::Parameter, palette));
    Some(len)
}

fn consume_simple_shell_variable(text: &str) -> Option<usize> {
    let mut chars = text.char_indices();
    let (_, dollar) = chars.next()?;
    if dollar != '$' {
        return None;
    }

    let Some((start, first)) = chars.next() else {
        return Some(1);
    };
    if matches!(first, '$' | '#' | '?' | '!' | '@' | '*' | '-' | '0'..='9') {
        return Some(start + first.len_utf8());
    }
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Some(1);
    }

    let mut end = start + first.len_utf8();
    for (offset, ch) in chars {
        if !(ch.is_ascii_alphanumeric() || ch == '_') {
            break;
        }
        end = offset + ch.len_utf8();
    }
    Some(end)
}

fn consume_balanced(text: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    for (offset, ch) in text.char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(offset + ch.len_utf8());
            }
        }
    }
    Some(text.len())
}

fn match_shell_punctuation(text: &str) -> Option<(usize, SemanticRole, bool, bool)> {
    for operator in [
        "<<-", "<<", ">>", "&&", "||", "<&", ">&", "<>", ">|", ";;", ";&", ";;&",
    ] {
        if text.starts_with(operator) {
            return Some((
                operator.len(),
                SemanticRole::Operator,
                matches!(operator, "&&" | "||" | ";;" | ";&" | ";;&"),
                operator.starts_with("<<"),
            ));
        }
    }

    let first = text.chars().next()?;
    let role = if matches!(first, '<' | '>' | ';' | '|' | '&' | '(' | ')' | '{' | '}') {
        SemanticRole::Operator
    } else {
        return None;
    };

    Some((
        first.len_utf8(),
        role,
        matches!(first, ';' | '|' | '{' | '('),
        false,
    ))
}

fn match_shell_assignment(text: &str) -> Option<(usize, usize)> {
    let mut chars = text.char_indices();
    let (_, first) = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }

    for (offset, ch) in chars {
        if ch == '=' {
            return Some((offset, offset + 1));
        }
        if !(ch.is_ascii_alphanumeric() || ch == '_') {
            return None;
        }
    }

    None
}

fn take_shell_word(text: &str) -> Option<usize> {
    let mut len = 0;
    for ch in text.chars() {
        if ch.is_ascii_whitespace()
            || matches!(
                ch,
                '#' | '"' | '\'' | '$' | ';' | '|' | '&' | '<' | '>' | '(' | ')' | '{' | '}'
            )
        {
            break;
        }
        len += ch.len_utf8();
    }
    (len > 0).then_some(len)
}

fn classify_shell_word(word: &str, state: &ShellLineState) -> SemanticRole {
    if state.expect_heredoc_marker {
        return SemanticRole::Parameter;
    }
    if is_shell_reserved_word(word) {
        return SemanticRole::Keyword;
    }
    if looks_like_shell_option(word) {
        return SemanticRole::Parameter;
    }
    if is_shell_builtin(word) {
        return SemanticRole::Function;
    }
    if state.command_position && looks_like_shell_command_name(word) {
        return SemanticRole::Function;
    }
    SemanticRole::Fg
}

fn advance_shell_state(
    mut state: ShellLineState,
    word: &str,
    role: SemanticRole,
) -> ShellLineState {
    state.at_line_start = false;

    if state.expect_heredoc_marker && !word.is_empty() {
        state.expect_heredoc_marker = false;
        state.command_position = false;
        return state;
    }

    if role == SemanticRole::Keyword {
        state.command_position = matches!(word, "if" | "then" | "elif" | "else" | "do");
        return state;
    }

    if role == SemanticRole::Function && is_shell_builtin(word) {
        state.command_position = matches!(
            word,
            "export" | "readonly" | "local" | "declare" | "typeset"
        );
        return state;
    }

    state.command_position = false;
    state
}

fn is_shell_reserved_word(word: &str) -> bool {
    matches!(
        word,
        "if" | "then"
            | "fi"
            | "for"
            | "do"
            | "done"
            | "case"
            | "esac"
            | "while"
            | "until"
            | "in"
            | "elif"
            | "else"
            | "select"
            | "function"
            | "time"
    )
}

fn is_shell_builtin(word: &str) -> bool {
    matches!(
        word,
        "[" | "]"
            | "."
            | "alias"
            | "bg"
            | "bind"
            | "builtin"
            | "caller"
            | "cd"
            | "command"
            | "compgen"
            | "complete"
            | "compopt"
            | "declare"
            | "dirs"
            | "disown"
            | "echo"
            | "enable"
            | "eval"
            | "exec"
            | "exit"
            | "export"
            | "false"
            | "fc"
            | "fg"
            | "getopts"
            | "hash"
            | "help"
            | "history"
            | "jobs"
            | "kill"
            | "let"
            | "local"
            | "mapfile"
            | "popd"
            | "printf"
            | "pushd"
            | "pwd"
            | "read"
            | "readarray"
            | "readonly"
            | "return"
            | "set"
            | "shift"
            | "shopt"
            | "source"
            | "suspend"
            | "test"
            | "times"
            | "trap"
            | "true"
            | "type"
            | "typeset"
            | "ulimit"
            | "umask"
            | "unalias"
            | "unset"
            | "wait"
    )
}

fn looks_like_shell_option(word: &str) -> bool {
    word.len() > 1 && word.starts_with('-')
}
