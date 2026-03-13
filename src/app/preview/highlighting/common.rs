use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

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

pub(super) fn split_block_comment_segments<'a>(
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

pub(super) fn scan_string(input: &str, start: usize, quote: char) -> usize {
    let mut index = start + quote.len_utf8();
    let mut escape = false;

    while let Some(ch) = input[index..].chars().next() {
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
            return index + ch.len_utf8();
        }
        index += ch.len_utf8();
    }

    input.len()
}

pub(super) fn next_non_whitespace_char(input: &str, start: usize) -> Option<char> {
    input[start..].chars().find(|ch| !ch.is_whitespace())
}

pub(super) fn consume_operator(input: &str, start: usize) -> usize {
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

pub(super) fn scan_make_variable(input: &str, start: usize) -> usize {
    let opener = input[start..].chars().nth(1).unwrap_or('(');
    let closer = if opener == '{' { '}' } else { ')' };
    let mut index = start + 2;
    let mut depth = 1usize;

    while index < input.len() {
        let ch = input[index..].chars().next().unwrap_or(closer);
        index += ch.len_utf8();
        if ch == opener {
            depth += 1;
        } else if ch == closer {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return index;
            }
        }
    }

    input.len()
}

pub(super) fn find_unquoted_token(input: &str, token: &str) -> Option<usize> {
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

        if input[index..].starts_with(token) {
            return Some(index);
        }

        if input[index..].starts_with("$(") || input[index..].starts_with("${") {
            index = scan_make_variable(input, index);
            continue;
        }

        index += ch.len_utf8();
    }

    None
}

pub(super) fn looks_numeric(token: &str) -> bool {
    let stripped = token.trim_matches(',');
    !stripped.is_empty()
        && stripped
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '_' | '-' | '+' | 'x' | 'o' | 'b'))
}

pub(super) fn styled_text(text: &str, color: Color, modifier: Modifier) -> Span<'static> {
    Span::styled(
        text.to_string(),
        Style::default().fg(color).add_modifier(modifier),
    )
}
