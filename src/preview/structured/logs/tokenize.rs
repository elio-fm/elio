pub(super) fn tokenize_preserving_quotes(line: &str) -> Vec<String> {
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

pub(super) fn consume_timestamp(tokens: &[String], index: &mut usize) -> Option<String> {
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

pub(super) fn consume_level(tokens: &[String], index: &mut usize) -> Option<String> {
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

pub(super) fn canonical_level(token: &str) -> Option<String> {
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

pub(super) fn parse_field_token(token: &str) -> Option<(String, String)> {
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

pub(super) fn is_level_field(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "level" | "lvl" | "severity" | "log.level"
    )
}

pub(super) fn is_timestamp_field(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "ts" | "time" | "timestamp" | "@timestamp"
    )
}

fn parse_level_token(token: &str) -> Option<String> {
    let cleaned = clean_wrapped(token);
    let cleaned = cleaned.trim_end_matches([':', ';', ',']);
    canonical_level(cleaned)
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
