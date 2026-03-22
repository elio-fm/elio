use super::render::truncate_display;
use super::types::{LogSource, ParsedLogDocument, ParsedLogEntry};
use serde_json::Value;

pub(super) fn parse_json_log_document(text: &str) -> Option<ParsedLogDocument> {
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
            level = json_scalar_to_string(&value)
                .and_then(|value| super::tokenize::canonical_level(&value));
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

#[cfg(test)]
mod tests {
    use super::super::render_log_preview;

    fn rendered_preview(text: &str) -> String {
        render_log_preview(text)
            .expect("structured log preview should render")
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn json_logs_render_as_structured_entries() {
        let rendered = rendered_preview(
            "{\"timestamp\":\"2026-03-10T12:00:00Z\",\"level\":\"info\",\"message\":\"started\",\"service\":\"api\"}\n\
             {\"timestamp\":\"2026-03-10T12:00:01Z\",\"level\":\"error\",\"message\":\"failed\",\"request_id\":42}\n",
        );

        assert!(rendered.contains("JSON lines"));
        assert!(rendered.contains("started"));
        assert!(rendered.contains("service"));
        assert!(rendered.contains("ERROR"));
    }

    #[test]
    fn json_logs_accept_alias_fields_and_stringify_nested_values() {
        let rendered = rendered_preview(
            "{\"@timestamp\":\"2026-03-10T12:00:00Z\",\"severity\":\"warning\",\"summary\":\"cache miss\",\"http\":{\"path\":\"/login\",\"status\":404}}\n",
        );

        assert!(rendered.contains("JSON lines"));
        assert!(rendered.contains("WARN"));
        assert!(rendered.contains("cache miss"));
        assert!(rendered.contains("http"));
        assert!(rendered.contains("/login"));
    }
}
