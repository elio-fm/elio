use super::{LINE_LIMIT, StructuredPreview, styled};
use crate::{appearance, file_facts::StructuredFormat};
use ratatui::{
    style::Modifier,
    text::{Line, Span},
};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn render_log_preview(text: &str) -> Option<StructuredPreview> {
    if text.trim().is_empty() {
        return Some(StructuredPreview {
            lines: vec![Line::from("File is empty")],
            detail: StructuredFormat::Log.detail_label(),
            truncation_note: None,
        });
    }

    let parsed = parse_json_log_document(text)
        .or_else(|| parse_access_log_document(text))
        .or_else(|| parse_general_log_document(text))?;
    Some(render_parsed_log(parsed))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LogSource {
    Json,
    Access,
    General,
}

impl LogSource {
    fn label(self) -> &'static str {
        match self {
            Self::Json => "JSON lines",
            Self::Access => "Access log",
            Self::General => "Application log",
        }
    }
}

#[derive(Clone, Debug)]
struct ParsedLogDocument {
    source: LogSource,
    entries: Vec<ParsedLogEntry>,
}

#[derive(Clone, Debug, Default)]
struct ParsedLogEntry {
    timestamp: Option<String>,
    level: Option<String>,
    message: String,
    fields: Vec<(String, String)>,
    continuations: Vec<String>,
}

#[derive(Clone, Debug)]
struct RawLogEntry {
    line: String,
    continuations: Vec<String>,
}

fn render_parsed_log(document: ParsedLogDocument) -> StructuredPreview {
    let palette = appearance::code_preview_palette();
    let mut counts = BTreeMap::new();
    for entry in &document.entries {
        if let Some(level) = &entry.level {
            *counts.entry(level.clone()).or_insert(0usize) += 1;
        }
    }

    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        styled("format", palette.parameter, Modifier::BOLD),
        styled(": ", palette.operator, Modifier::empty()),
        Span::raw(document.source.label().to_string()),
        Span::raw("  ".to_string()),
        styled("entries", palette.parameter, Modifier::BOLD),
        styled(": ", palette.operator, Modifier::empty()),
        Span::raw(document.entries.len().to_string()),
    ]));

    if let Some((first, last)) = time_range(&document.entries) {
        let mut spans = vec![
            styled("range", palette.parameter, Modifier::BOLD),
            styled(": ", palette.operator, Modifier::empty()),
            styled(&first, palette.comment, Modifier::empty()),
        ];
        if first != last {
            spans.push(Span::raw("  ->  ".to_string()));
            spans.push(styled(&last, palette.comment, Modifier::empty()));
        }
        lines.push(Line::from(spans));
    }

    if !counts.is_empty() {
        let mut spans = vec![
            styled("levels", palette.parameter, Modifier::BOLD),
            styled(": ", palette.operator, Modifier::empty()),
        ];
        for (index, (level, count)) in counts.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw("  ".to_string()));
            }
            spans.push(styled(
                level,
                log_level_color(level, palette),
                Modifier::BOLD,
            ));
            spans.push(Span::raw(format!(" {count}")));
        }
        lines.push(Line::from(spans));
    }

    if !lines.is_empty() {
        lines.push(Line::from(""));
    }

    let mut truncated = false;
    for entry in document.entries {
        if lines.len() >= LINE_LIMIT {
            truncated = true;
            break;
        }
        lines.push(render_entry_summary(&entry, palette));

        for (key, value) in entry.fields {
            if lines.len() >= LINE_LIMIT {
                truncated = true;
                break;
            }
            lines.push(Line::from(vec![
                Span::raw("  ".to_string()),
                styled(&key, palette.parameter, Modifier::BOLD),
                styled(": ", palette.operator, Modifier::empty()),
                Span::raw(truncate_display(&value, 96)),
            ]));
        }
        if truncated {
            break;
        }

        for continuation in entry.continuations {
            if lines.len() >= LINE_LIMIT {
                truncated = true;
                break;
            }
            lines.push(Line::from(vec![
                Span::raw("  ".to_string()),
                styled("│", palette.comment, Modifier::empty()),
                Span::raw(" ".to_string()),
                styled(
                    &truncate_display(&continuation, 116),
                    palette.comment,
                    Modifier::empty(),
                ),
            ]));
        }
    }

    StructuredPreview {
        lines,
        detail: StructuredFormat::Log.detail_label(),
        truncation_note: truncated.then(|| format!("showing first {LINE_LIMIT} lines")),
    }
}

fn render_entry_summary(
    entry: &ParsedLogEntry,
    palette: appearance::CodePreviewPalette,
) -> Line<'static> {
    let mut spans = Vec::new();
    if let Some(timestamp) = &entry.timestamp {
        spans.push(styled(timestamp, palette.comment, Modifier::empty()));
        spans.push(Span::raw("  ".to_string()));
    }
    if let Some(level) = &entry.level {
        spans.push(styled(
            level,
            log_level_color(level, palette),
            Modifier::BOLD,
        ));
        spans.push(Span::raw("  ".to_string()));
    }
    spans.push(Span::raw(truncate_display(&entry.message, 116)));
    Line::from(spans)
}

fn time_range(entries: &[ParsedLogEntry]) -> Option<(String, String)> {
    let first = entries.iter().find_map(|entry| entry.timestamp.clone())?;
    let last = entries
        .iter()
        .rev()
        .find_map(|entry| entry.timestamp.clone())
        .unwrap_or_else(|| first.clone());
    Some((first, last))
}

fn parse_json_log_document(text: &str) -> Option<ParsedLogDocument> {
    let mut entries = Vec::new();
    let mut non_empty = 0usize;
    let mut parsed = 0usize;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        non_empty += 1;
        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        let Some(entry) = parse_json_log_value(value) else {
            continue;
        };
        parsed += 1;
        entries.push(entry);
    }

    if non_empty == 0 || parsed == 0 || parsed * 100 < non_empty * 80 {
        return None;
    }

    Some(ParsedLogDocument {
        source: LogSource::Json,
        entries,
    })
}

fn parse_json_log_value(value: Value) -> Option<ParsedLogEntry> {
    let Value::Object(map) = value else {
        return None;
    };

    let mut timestamp = None;
    let mut level = None;
    let mut message = None;
    let mut fields = Vec::new();

    for (key, value) in map {
        let normalized = key.to_ascii_lowercase();
        if timestamp.is_none()
            && matches!(
                normalized.as_str(),
                "ts" | "time" | "timestamp" | "@timestamp"
            )
        {
            timestamp = json_scalar_to_string(&value);
            continue;
        }
        if level.is_none()
            && matches!(
                normalized.as_str(),
                "level" | "lvl" | "severity" | "log.level"
            )
        {
            level = json_scalar_to_string(&value).and_then(|value| canonical_level(&value));
            continue;
        }
        if message.is_none()
            && matches!(
                normalized.as_str(),
                "msg" | "message" | "event" | "error" | "summary"
            )
        {
            message = json_scalar_to_string(&value);
            continue;
        }

        if let Some(stringified) = json_value_to_field(&value) {
            fields.push((key, stringified));
        }
    }

    if timestamp.is_none() && level.is_none() && message.is_none() && fields.is_empty() {
        return None;
    }

    Some(ParsedLogEntry {
        timestamp,
        level,
        message: message.unwrap_or_else(|| "JSON log entry".to_string()),
        fields,
        continuations: Vec::new(),
    })
}

fn parse_access_log_document(text: &str) -> Option<ParsedLogDocument> {
    let mut entries = Vec::new();
    let mut non_empty = 0usize;
    let mut parsed = 0usize;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        non_empty += 1;
        let Some(entry) = parse_access_log_line(trimmed) else {
            continue;
        };
        parsed += 1;
        entries.push(entry);
    }

    if non_empty == 0 || parsed == 0 || parsed * 100 < non_empty * 70 {
        return None;
    }

    Some(ParsedLogDocument {
        source: LogSource::Access,
        entries,
    })
}

fn parse_access_log_line(line: &str) -> Option<ParsedLogEntry> {
    let mut rest = line.trim();
    let host = take_token(&mut rest)?;
    let ident = take_token(&mut rest)?;
    let user = take_token(&mut rest)?;
    let timestamp = take_wrapped(&mut rest, '[', ']')?;
    let request = take_wrapped(&mut rest, '"', '"')?;
    let status = take_token(&mut rest)?;
    let bytes = take_token(&mut rest)?;
    let referer = take_optional_wrapped(&mut rest, '"', '"');
    let user_agent = take_optional_wrapped(&mut rest, '"', '"');

    let method = request.split_whitespace().next().unwrap_or("REQUEST");
    let path = request
        .split_whitespace()
        .nth(1)
        .unwrap_or(request.as_str())
        .to_string();
    let level = access_status_level(&status);
    let mut fields = vec![
        ("host".to_string(), host),
        ("status".to_string(), status),
        ("bytes".to_string(), bytes),
    ];
    if ident != "-" {
        fields.push(("ident".to_string(), ident));
    }
    if user != "-" {
        fields.push(("user".to_string(), user));
    }
    if let Some(referer) = referer
        && referer != "-"
    {
        fields.push(("referer".to_string(), referer));
    }
    if let Some(user_agent) = user_agent
        && user_agent != "-"
    {
        fields.push(("user-agent".to_string(), user_agent));
    }

    Some(ParsedLogEntry {
        timestamp: Some(timestamp),
        level,
        message: format!("{method} {path}"),
        fields,
        continuations: Vec::new(),
    })
}

fn parse_general_log_document(text: &str) -> Option<ParsedLogDocument> {
    let raw_entries = group_general_log_lines(text);
    if raw_entries.is_empty() {
        return None;
    }

    let mut entries = Vec::new();
    let mut structured_entries = 0usize;
    let mut total_signal = 0usize;

    for raw_entry in raw_entries {
        let parsed = parse_general_log_entry(raw_entry);
        if parsed.signal_count > 0 {
            structured_entries += 1;
        }
        total_signal += parsed.signal_count;
        entries.push(parsed.entry);
    }

    if structured_entries == 0
        || structured_entries * 100 < entries.len() * 60
        || total_signal < entries.len()
    {
        return None;
    }

    Some(ParsedLogDocument {
        source: LogSource::General,
        entries,
    })
}

fn group_general_log_lines(text: &str) -> Vec<RawLogEntry> {
    let mut grouped = Vec::new();
    let mut current: Option<RawLogEntry> = None;

    for line in text.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            if let Some(current) = &mut current
                && !current.continuations.is_empty()
            {
                current.continuations.push(String::new());
            }
            continue;
        }

        let starts_new = starts_general_entry(trimmed);
        if let Some(existing) = &mut current {
            if starts_new {
                grouped.push(existing.clone());
                current = Some(RawLogEntry {
                    line: trimmed.to_string(),
                    continuations: Vec::new(),
                });
                continue;
            }

            if looks_like_continuation(line) {
                existing
                    .continuations
                    .push(trimmed.trim_start().to_string());
                continue;
            }

            grouped.push(existing.clone());
        }

        current = Some(RawLogEntry {
            line: trimmed.to_string(),
            continuations: Vec::new(),
        });
    }

    if let Some(current) = current {
        grouped.push(current);
    }
    grouped
}

struct GeneralParse {
    entry: ParsedLogEntry,
    signal_count: usize,
}

fn parse_general_log_entry(raw: RawLogEntry) -> GeneralParse {
    let tokens = tokenize_preserving_quotes(&raw.line);
    let mut index = 0usize;
    let timestamp = consume_timestamp(&tokens, &mut index);
    let mut level = consume_level(&tokens, &mut index);
    let mut fields = Vec::new();
    let mut message_parts = Vec::new();

    for token in &tokens[index..] {
        if let Some((key, value)) = parse_field_token(token) {
            if level.is_none() && is_level_field(&key) {
                level = canonical_level(&value);
                continue;
            }
            if is_timestamp_field(&key) {
                continue;
            }
            fields.push((key, value));
        } else {
            message_parts.push(token.to_string());
        }
    }

    let message = if message_parts.is_empty() {
        raw.line.clone()
    } else {
        message_parts.join(" ")
    };
    let signal_count = usize::from(timestamp.is_some())
        + usize::from(level.is_some())
        + usize::from(!fields.is_empty())
        + usize::from(!raw.continuations.is_empty());

    GeneralParse {
        entry: ParsedLogEntry {
            timestamp,
            level,
            message,
            fields,
            continuations: raw.continuations,
        },
        signal_count,
    }
}

fn starts_general_entry(line: &str) -> bool {
    let tokens = tokenize_preserving_quotes(line);
    if tokens.is_empty() {
        return false;
    }

    let mut index = 0usize;
    if consume_timestamp(&tokens, &mut index).is_some() {
        return true;
    }

    consume_level(&tokens, &mut index).is_some()
        || tokens
            .iter()
            .take(4)
            .filter_map(|token| parse_field_token(token))
            .any(|(key, _)| is_level_field(&key) || is_timestamp_field(&key))
}

fn looks_like_continuation(line: &str) -> bool {
    let trimmed = line.trim_start();
    line.starts_with(char::is_whitespace)
        || matches!(
            trimmed,
            text if text.starts_with("at ")
                || text.starts_with("Caused by:")
                || text.starts_with("Traceback")
                || text.starts_with("File \"")
                || text.starts_with("...")
                || text.starts_with("Stack trace:")
        )
}

fn consume_timestamp(tokens: &[String], index: &mut usize) -> Option<String> {
    let token = tokens.get(*index)?;
    if looks_like_single_timestamp(token) {
        *index += 1;
        return Some(clean_wrapped(token));
    }

    let second = tokens.get(*index + 1)?;
    if looks_like_date_token(token) && looks_like_time_token(second) {
        let mut timestamp = format!("{} {}", clean_wrapped(token), clean_wrapped(second));
        *index += 2;
        if let Some(third) = tokens.get(*index)
            && looks_like_timezone_token(third)
        {
            timestamp.push(' ');
            timestamp.push_str(&clean_wrapped(third));
            *index += 1;
        }
        return Some(timestamp);
    }

    let third = tokens.get(*index + 2)?;
    if looks_like_month_token(token)
        && second.chars().all(|ch| ch.is_ascii_digit())
        && looks_like_time_token(third)
    {
        *index += 3;
        return Some(format!(
            "{} {} {}",
            clean_wrapped(token),
            clean_wrapped(second),
            clean_wrapped(third)
        ));
    }

    None
}

fn consume_level(tokens: &[String], index: &mut usize) -> Option<String> {
    let token = tokens.get(*index)?;
    if let Some(level) = parse_level_token(token) {
        *index += 1;
        return Some(level);
    }

    if let Some((key, value)) = parse_field_token(token)
        && is_level_field(&key)
    {
        *index += 1;
        return canonical_level(&value);
    }

    None
}

fn parse_level_token(token: &str) -> Option<String> {
    let cleaned = clean_wrapped(token);
    let cleaned = cleaned.trim_end_matches([':', ';', ',']);
    canonical_level(cleaned)
}

fn canonical_level(token: &str) -> Option<String> {
    match token
        .trim_matches(['[', ']', '(', ')'])
        .to_ascii_uppercase()
        .as_str()
    {
        "TRACE" => Some("TRACE".to_string()),
        "DEBUG" => Some("DEBUG".to_string()),
        "INFO" | "NOTICE" => Some("INFO".to_string()),
        "WARN" | "WARNING" | "WRN" => Some("WARN".to_string()),
        "ERROR" | "ERR" => Some("ERROR".to_string()),
        "FATAL" | "CRITICAL" | "CRIT" | "ALERT" | "EMERG" => Some("FATAL".to_string()),
        _ => None,
    }
}

fn parse_field_token(token: &str) -> Option<(String, String)> {
    for separator in ['=', ':'] {
        let Some((left, right)) = token.split_once(separator) else {
            continue;
        };
        if right.is_empty() || !looks_like_field_key(left) {
            continue;
        }
        if separator == ':' && right.starts_with("//") {
            continue;
        }
        return Some((left.to_string(), normalize_field_value(right)));
    }
    None
}

fn looks_like_field_key(key: &str) -> bool {
    let Some(first) = key.chars().next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '@'))
}

fn normalize_field_value(value: &str) -> String {
    let trimmed = value.trim_matches(|ch| matches!(ch, '"' | '\'' | '[' | ']'));
    trimmed.to_string()
}

fn tokenize_preserving_quotes(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for ch in line.chars() {
        if let Some(active_quote) = quote {
            current.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' && active_quote == '"' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }

        if matches!(ch, '"' | '\'') {
            quote = Some(ch);
            current.push(ch);
            continue;
        }

        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }

        current.push(ch);
    }

    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn take_token(input: &mut &str) -> Option<String> {
    *input = input.trim_start();
    let end = input.find(char::is_whitespace).unwrap_or(input.len());
    if end == 0 {
        return None;
    }
    let token = input[..end].to_string();
    *input = &input[end..];
    Some(token)
}

fn take_wrapped(input: &mut &str, open: char, close: char) -> Option<String> {
    *input = input.trim_start();
    let trimmed = *input;
    if !trimmed.starts_with(open) {
        return None;
    }
    let end = trimmed[1..].find(close)? + 1;
    let content = trimmed[1..end].to_string();
    *input = &trimmed[end + 1..];
    Some(content)
}

fn take_optional_wrapped(input: &mut &str, open: char, close: char) -> Option<String> {
    let trimmed = input.trim_start();
    if trimmed.is_empty() || !trimmed.starts_with(open) {
        *input = trimmed;
        return None;
    }
    take_wrapped(input, open, close)
}

fn access_status_level(status: &str) -> Option<String> {
    match status.parse::<u16>().ok()? / 100 {
        5 => Some("ERROR".to_string()),
        4 => Some("WARN".to_string()),
        1..=3 => Some("INFO".to_string()),
        _ => None,
    }
}

fn json_scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(string) => Some(string.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        _ => None,
    }
}

fn json_value_to_field(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(string) => Some(string.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        other => Some(truncate_display(&other.to_string(), 96)),
    }
}

fn clean_wrapped(token: &str) -> String {
    token
        .trim_matches(|ch| matches!(ch, '[' | ']' | '(' | ')'))
        .to_string()
}

fn looks_like_single_timestamp(token: &str) -> bool {
    let cleaned = clean_wrapped(token);
    looks_like_timestamp_token(&cleaned)
}

fn looks_like_timestamp_token(token: &str) -> bool {
    token.len() >= 8
        && token.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        && token.contains(':')
        && (token.contains('-')
            || token.contains('/')
            || token.contains('T')
            || token.ends_with('Z')
            || token.contains('.')
            || token.contains(','))
}

fn looks_like_date_token(token: &str) -> bool {
    let cleaned = clean_wrapped(token);
    cleaned.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        && (cleaned.contains('-') || cleaned.contains('/'))
}

fn looks_like_time_token(token: &str) -> bool {
    let cleaned = clean_wrapped(token);
    cleaned.contains(':')
        && cleaned
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, ':' | '.' | ',' | 'Z'))
}

fn looks_like_timezone_token(token: &str) -> bool {
    let cleaned = clean_wrapped(token);
    (cleaned.starts_with('+') || cleaned.starts_with('-'))
        && cleaned[1..]
            .chars()
            .all(|ch| ch.is_ascii_digit() || ch == ':')
}

fn looks_like_month_token(token: &str) -> bool {
    matches!(
        clean_wrapped(token).to_ascii_lowercase().as_str(),
        "jan"
            | "feb"
            | "mar"
            | "apr"
            | "may"
            | "jun"
            | "jul"
            | "aug"
            | "sep"
            | "oct"
            | "nov"
            | "dec"
    )
}

fn is_level_field(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "level" | "lvl" | "severity" | "log.level"
    )
}

fn is_timestamp_field(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "ts" | "time" | "timestamp" | "@timestamp"
    )
}

fn truncate_display(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }

    let kept = value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    format!("{kept}…")
}

fn log_level_color(level: &str, palette: appearance::CodePreviewPalette) -> ratatui::style::Color {
    match level {
        "TRACE" | "DEBUG" => palette.comment,
        "INFO" => palette.function,
        "WARN" => palette.constant,
        "ERROR" | "FATAL" => palette.invalid,
        _ => palette.keyword,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_logs_render_as_structured_entries() {
        let preview = render_log_preview(
            "{\"timestamp\":\"2026-03-10T12:00:00Z\",\"level\":\"info\",\"message\":\"started\",\"service\":\"api\"}\n\
             {\"timestamp\":\"2026-03-10T12:00:01Z\",\"level\":\"error\",\"message\":\"failed\",\"request_id\":42}\n",
        )
        .expect("json logs should render");

        let rendered = preview
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("JSON lines"));
        assert!(rendered.contains("started"));
        assert!(rendered.contains("service"));
        assert!(rendered.contains("ERROR"));
    }

    #[test]
    fn access_logs_are_detected_and_summarized() {
        let preview = render_log_preview(
            "127.0.0.1 - - [10/Mar/2026:12:00:00 +0000] \"GET /login HTTP/1.1\" 500 123 \"-\" \"curl/8.0\"\n",
        )
        .expect("access log should render");

        let rendered = preview
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Access log"));
        assert!(rendered.contains("GET /login"));
        assert!(rendered.contains("status"));
        assert!(rendered.contains("500"));
    }

    #[test]
    fn multiline_logs_keep_stack_traces_attached() {
        let preview = render_log_preview(
            "2026-03-10T12:00:00Z ERROR request_id=42 msg=\"request failed\"\n\
                at service.handle (/srv/app.js:10)\n\
                Caused by: timeout\n\
             2026-03-10T12:00:01Z INFO request_id=42 recovered\n",
        )
        .expect("multiline log should render");

        let rendered = preview
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("request failed"));
        assert!(rendered.contains("Caused by: timeout"));
        assert!(rendered.contains("recovered"));
    }

    #[test]
    fn lower_case_and_bracketed_levels_are_normalized() {
        let preview = render_log_preview(
            "2026-03-10 12:00:00 [warn] request_id=42 delayed\n\
             2026-03-10 12:00:01 level=error request_id=42 failed\n",
        )
        .expect("normalized levels should render");

        let rendered = preview
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("WARN"));
        assert!(rendered.contains("ERROR"));
    }

    #[test]
    fn unstructured_logs_return_none_for_structured_rendering() {
        assert!(
            render_log_preview("starting application\nloading configuration\nready\n").is_none()
        );
    }
}
