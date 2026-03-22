use super::tokenize::{
    consume_level, consume_timestamp, is_level_field, is_timestamp_field, parse_field_token,
    tokenize_preserving_quotes,
};
use super::types::{LogSource, ParsedLogDocument, ParsedLogEntry, RawLogEntry};

pub(super) fn parse_general_log_document(text: &str) -> Option<ParsedLogDocument> {
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
                level = super::tokenize::canonical_level(&value);
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

#[cfg(test)]
mod tests {
    use super::super::render_log_preview;

    fn rendered_preview(text: &str) -> String {
        render_log_preview(text)
            .expect("general log preview should render")
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn multiline_logs_keep_stack_traces_attached() {
        let rendered = rendered_preview(
            "2026-03-10T12:00:00Z ERROR request_id=42 msg=\"request failed\"\n\
                at service.handle (/srv/app.js:10)\n\
                Caused by: timeout\n\
             2026-03-10T12:00:01Z INFO request_id=42 recovered\n",
        );

        assert!(rendered.contains("request failed"));
        assert!(rendered.contains("Caused by: timeout"));
        assert!(rendered.contains("recovered"));
    }

    #[test]
    fn lower_case_and_bracketed_levels_are_normalized() {
        let rendered = rendered_preview(
            "2026-03-10 12:00:00 [warn] request_id=42 delayed\n\
             2026-03-10 12:00:01 level=error request_id=42 failed\n",
        );

        assert!(rendered.contains("WARN"));
        assert!(rendered.contains("ERROR"));
    }

    #[test]
    fn general_logs_preserve_quoted_field_values_and_month_timestamps() {
        let rendered = rendered_preview(
            "Mar 10 12:00:00 level=info request_id=42 msg=\"cache rebuilt successfully\"\n",
        );

        assert!(rendered.contains("Application log"));
        assert!(rendered.contains("Mar 10 12:00:00"));
        assert!(rendered.contains("INFO"));
        assert!(rendered.contains("request_id"));
        assert!(rendered.contains("cache rebuilt successfully"));
    }
}
