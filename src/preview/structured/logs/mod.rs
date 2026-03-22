mod access;
mod json;
mod render;
mod types;

use self::access::parse_access_log_document;
use self::json::parse_json_log_document;
use self::render::render_parsed_log;
use self::types::{LogSource, ParsedLogDocument, ParsedLogEntry, RawLogEntry};
use super::StructuredPreview;

pub(super) fn render_log_preview(text: &str) -> Option<StructuredPreview> {
    if text.trim().is_empty() {
        return Some(StructuredPreview {
            lines: vec![ratatui::text::Line::from("File is empty")],
            detail: crate::file_info::StructuredFormat::Log.detail_label(),
            truncation_note: None,
        });
    }

    let parsed = parse_json_log_document(text)
        .or_else(|| parse_access_log_document(text))
        .or_else(|| parse_general_log_document(text))?;
    Some(render_parsed_log(parsed))
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
    fn access_logs_keep_optional_referer_and_user_agent_fields() {
        let preview = render_log_preview(
            "127.0.0.1 app elio [10/Mar/2026:12:00:00 +0000] \"GET /login HTTP/1.1\" 404 321 \"https://elio.dev/docs\" \"Mozilla/5.0\"\n",
        )
        .expect("access log with optional fields should render");

        let rendered = preview
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("WARN"));
        assert!(rendered.contains("ident"));
        assert!(rendered.contains("elio"));
        assert!(rendered.contains("referer"));
        assert!(rendered.contains("https://elio.dev/docs"));
        assert!(rendered.contains("user-agent"));
        assert!(rendered.contains("Mozilla/5.0"));
    }

    #[test]
    fn json_logs_accept_alias_fields_and_stringify_nested_values() {
        let preview = render_log_preview(
            "{\"@timestamp\":\"2026-03-10T12:00:00Z\",\"severity\":\"warning\",\"summary\":\"cache miss\",\"http\":{\"path\":\"/login\",\"status\":404}}\n",
        )
        .expect("json alias fields should render");

        let rendered = preview
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("JSON lines"));
        assert!(rendered.contains("WARN"));
        assert!(rendered.contains("cache miss"));
        assert!(rendered.contains("http"));
        assert!(rendered.contains("/login"));
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
    fn general_logs_preserve_quoted_field_values_and_month_timestamps() {
        let preview = render_log_preview(
            "Mar 10 12:00:00 level=info request_id=42 msg=\"cache rebuilt successfully\"\n",
        )
        .expect("general log with quoted fields should render");

        let rendered = preview
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Application log"));
        assert!(rendered.contains("Mar 10 12:00:00"));
        assert!(rendered.contains("INFO"));
        assert!(rendered.contains("request_id"));
        assert!(rendered.contains("cache rebuilt successfully"));
    }

    #[test]
    fn unstructured_logs_return_none_for_structured_rendering() {
        assert!(
            render_log_preview("starting application\nloading configuration\nready\n").is_none()
        );
    }
}
