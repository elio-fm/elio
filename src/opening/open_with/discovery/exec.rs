#[cfg(all(unix, not(target_os = "macos")))]
use std::path::Path;

/// Expands the `Exec=` field from a .desktop file into `(program, args)`.
///
/// Supported placeholders: `%f`, `%F`, `%u`, `%U` → replaced with the target
/// file path.  `%i`, `%c`, `%k` are stripped.  Unknown `%x` sequences are
/// dropped.
#[cfg(all(unix, not(target_os = "macos")))]
pub(super) fn expand_exec_template(exec: &str, target: &Path) -> Option<(String, Vec<String>)> {
    let target_str = target.to_str()?;
    let tokens = tokenize_exec(exec);

    let mut expanded: Vec<String> = Vec::new();
    for token in tokens {
        match token.as_str() {
            // Strip deprecated / icon / class / location placeholders.
            "%i" | "%c" | "%k" => {}
            // Standalone file/URL placeholders — replace with the single target.
            "%f" | "%F" | "%u" | "%U" => expanded.push(target_str.to_string()),
            other => {
                // Replace known placeholders embedded inside a larger token
                // (e.g. --file=%f), then strip any remaining unknown %x codes
                // so they are never passed to the child process.
                let replaced = other
                    .replace("%f", target_str)
                    .replace("%F", target_str)
                    .replace("%u", target_str)
                    .replace("%U", target_str)
                    .replace("%i", "")
                    .replace("%c", "")
                    .replace("%k", "");
                let clean = strip_unknown_field_codes(&replaced);
                if !clean.is_empty() {
                    expanded.push(clean);
                }
            }
        }
    }

    if expanded.is_empty() {
        return None;
    }

    let program = expanded.remove(0);
    Some((program, expanded))
}

/// Removes any `%x` field codes that were not already handled, so they are
/// never forwarded to the child process.  `%%` is converted to a literal `%`.
#[cfg(all(unix, not(target_os = "macos")))]
fn strip_unknown_field_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            match chars.peek() {
                Some('%') => {
                    chars.next();
                    result.push('%');
                }
                Some(_) => {
                    chars.next(); // drop %x
                }
                None => {} // trailing bare % — drop it
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Splits a desktop-spec Exec string into tokens, respecting double-quoted
/// strings and backslash escapes.
#[cfg(any(target_os = "macos", all(unix, not(target_os = "macos"))))]
pub(super) fn tokenize_exec(exec: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = exec.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => in_quotes = !in_quotes,
            '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

#[cfg(all(test, unix, not(target_os = "macos")))]
mod tests;
