use super::SymlinkInfo;
use std::{io, path::Path, time::SystemTime};

const SIZE_UNITS: [&str; 7] = ["B", "kB", "MB", "GB", "TB", "PB", "EB"];

pub(crate) fn display_path(path: &Path) -> String {
    strip_windows_verbatim_prefix(&path.display().to_string())
}

fn strip_windows_verbatim_prefix(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

pub(crate) fn format_size(size: u64) -> String {
    let (quantity, unit) = format_size_parts(size);
    format!("{quantity} {unit}")
}

pub(crate) fn format_size_parts(size: u64) -> (String, &'static str) {
    if size < 1000 {
        return (format_with_grouping(size), SIZE_UNITS[0]);
    }

    let mut value = size as f64;
    let mut unit = 0usize;
    while value >= 1000.0 && unit < SIZE_UNITS.len() - 1 {
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
    (format_decimal(value, precision), SIZE_UNITS[unit])
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

pub(crate) fn symlink_target_display_label(symlink: &SymlinkInfo) -> String {
    symlink
        .target
        .as_ref()
        .map(|path| sanitize_terminal_text(&path.display().to_string()))
        .unwrap_or_else(|| "unreadable target".to_string())
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
#[path = "tests/item_display.rs"]
mod tests;
