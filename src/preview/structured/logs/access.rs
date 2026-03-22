use super::types::{LogSource, ParsedLogDocument, ParsedLogEntry};

pub(super) fn parse_access_log_document(text: &str) -> Option<ParsedLogDocument> {
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

#[cfg(test)]
mod tests {
    use super::super::render_log_preview;

    fn rendered_preview(text: &str) -> String {
        render_log_preview(text)
            .expect("access log preview should render")
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn access_logs_are_detected_and_summarized() {
        let rendered = rendered_preview(
            "127.0.0.1 - - [10/Mar/2026:12:00:00 +0000] \"GET /login HTTP/1.1\" 500 123 \"-\" \"curl/8.0\"\n",
        );

        assert!(rendered.contains("Access log"));
        assert!(rendered.contains("GET /login"));
        assert!(rendered.contains("status"));
        assert!(rendered.contains("500"));
    }

    #[test]
    fn access_logs_keep_optional_referer_and_user_agent_fields() {
        let rendered = rendered_preview(
            "127.0.0.1 app elio [10/Mar/2026:12:00:00 +0000] \"GET /login HTTP/1.1\" 404 321 \"https://elio.dev/docs\" \"Mozilla/5.0\"\n",
        );

        assert!(rendered.contains("WARN"));
        assert!(rendered.contains("ident"));
        assert!(rendered.contains("elio"));
        assert!(rendered.contains("referer"));
        assert!(rendered.contains("https://elio.dev/docs"));
        assert!(rendered.contains("user-agent"));
        assert!(rendered.contains("Mozilla/5.0"));
    }
}
