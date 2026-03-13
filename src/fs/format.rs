use ratatui::layout::Rect;
use std::{io, time::SystemTime};

pub(crate) fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

pub(crate) fn format_size(size: u64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    if size < 1000 {
        return format!("{} B", format_with_grouping(size));
    }

    let mut value = size as f64;
    let mut unit = 0usize;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }

    let precision = if value < 10.0 {
        2
    } else if value < 100.0 {
        1
    } else {
        0
    };
    format!("{} {}", format_decimal(value, precision), UNITS[unit])
}

pub(crate) fn format_item_count(count: usize) -> String {
    match count {
        1 => "1 item".to_string(),
        _ => format!("{} items", format_with_grouping(count as u64)),
    }
}

pub(crate) fn format_time_ago(time: SystemTime) -> String {
    let Ok(age) = SystemTime::now().duration_since(time) else {
        return "just now".to_string();
    };
    let seconds = age.as_secs();
    match seconds {
        0..=59 => format!("{seconds}s ago"),
        60..=3599 => format!("{}m ago", seconds / 60),
        3600..=86_399 => format!("{}h ago", seconds / 3600),
        86_400..=2_592_000 => format!("{}d ago", seconds / 86_400),
        _ => format!("{}mo ago", seconds / 2_592_000),
    }
}

pub(crate) fn describe_io_error(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::PermissionDenied => "Permission denied",
        io::ErrorKind::NotFound => "Not found",
        io::ErrorKind::Unsupported => "Unsupported location",
        _ => "Read error",
    }
}

pub(crate) fn sanitize_terminal_text(text: &str) -> String {
    let mut sanitized = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\t' => sanitized.push_str("    "),
            '\u{0000}'..='\u{001f}' => {
                sanitized.push('^');
                sanitized.push((b'@' + ch as u8) as char);
            }
            '\u{007f}' => sanitized.push_str("^?"),
            ch if ch.is_control() => sanitized.push_str(&format!("\\u{{{:x}}}", ch as u32)),
            ch => sanitized.push(ch),
        }
    }
    sanitized
}

fn format_with_grouping(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped
}

fn format_decimal(value: f64, precision: usize) -> String {
    let mut formatted = format!("{value:.precision$}");
    if precision == 0 {
        return formatted;
    }

    while formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    formatted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_format_is_human_readable() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2_048), "2.05 kB");
        assert_eq!(format_size(5_488), "5.49 kB");
        assert_eq!(format_size(12_345_678), "12.3 MB");
    }

    #[test]
    fn item_count_format_uses_singular_and_grouping() {
        assert_eq!(format_item_count(1), "1 item");
        assert_eq!(format_item_count(24), "24 items");
        assert_eq!(format_item_count(1_234), "1,234 items");
    }

    #[test]
    fn terminal_text_is_sanitized_before_rendering() {
        assert_eq!(
            sanitize_terminal_text("bad\rname\t\u{1b}"),
            "bad^Mname    ^["
        );
    }
}
