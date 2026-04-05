use std::path::Path;

/// Expands the `Exec=` field from a .desktop file into `(program, args)`.
///
/// Supported placeholders: `%f`, `%F`, `%u`, `%U` → replaced with the target
/// file path.  `%i`, `%c`, `%k` are stripped.  Unknown `%x` sequences are
/// dropped.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ── expand_exec_template ──────────────────────────────────────────────────

    #[test]
    fn expand_exec_template_supports_percent_f_and_percent_u() {
        let path = Path::new("/home/user/doc.txt");

        let (prog, args) = expand_exec_template("gedit %f", path).expect("should expand");
        assert_eq!(prog, "gedit");
        assert_eq!(args, vec!["/home/user/doc.txt"]);

        let (prog, args) = expand_exec_template("vlc %u", path).expect("should expand");
        assert_eq!(prog, "vlc");
        assert_eq!(args, vec!["/home/user/doc.txt"]);
    }

    #[test]
    fn expand_exec_template_supports_uppercase_percent_f_and_percent_u() {
        let path = Path::new("/tmp/file.png");

        let (prog, args) = expand_exec_template("eog %F", path).expect("should expand");
        assert_eq!(prog, "eog");
        assert_eq!(args, vec!["/tmp/file.png"]);

        let (prog, args) = expand_exec_template("vlc %U", path).expect("should expand");
        assert_eq!(prog, "vlc");
        assert_eq!(args, vec!["/tmp/file.png"]);
    }

    #[test]
    fn expand_exec_template_strips_percent_i_percent_c_percent_k() {
        let path = Path::new("/tmp/x.txt");

        // %i, %c, %k as standalone tokens — should all be dropped.
        let (prog, args) = expand_exec_template("nano %i %c %k %f", path).expect("should expand");
        assert_eq!(prog, "nano");
        assert_eq!(args, vec!["/tmp/x.txt"]);
    }

    #[test]
    fn expand_exec_template_handles_embedded_placeholder() {
        let path = Path::new("/tmp/image.png");

        let (prog, args) =
            expand_exec_template("viewer --file=%f --quality=90", path).expect("should expand");
        assert_eq!(prog, "viewer");
        assert_eq!(args, vec!["--file=/tmp/image.png", "--quality=90"]);
    }

    #[test]
    fn expand_exec_template_handles_quoted_program() {
        let path = Path::new("/tmp/doc.txt");

        let (prog, args) = expand_exec_template(r#""my editor" %f"#, path).expect("should expand");
        assert_eq!(prog, "my editor");
        assert_eq!(args, vec!["/tmp/doc.txt"]);
    }

    #[test]
    fn expand_exec_template_returns_none_for_empty_after_strip() {
        let path = Path::new("/tmp/x");
        // Only stripped placeholders — nothing left.
        let result = expand_exec_template("%i %c %k", path);
        assert!(result.is_none());
    }

    #[test]
    fn expand_exec_template_drops_unknown_placeholders() {
        let path = Path::new("/tmp/doc.txt");

        // %d, %n, %D, %v, %m are deprecated/unknown — must not pass through.
        let (prog, args) =
            expand_exec_template("app %d %n %f", path).expect("should expand with file arg");
        assert_eq!(prog, "app");
        assert_eq!(args, vec!["/tmp/doc.txt"]);
    }

    #[test]
    fn expand_exec_template_handles_embedded_unknown_placeholder() {
        let path = Path::new("/tmp/img.png");

        // An embedded unknown code like %v inside an option should be stripped,
        // not forwarded to the program.
        let (prog, args) = expand_exec_template("viewer --opt=%v %f", path).expect("should expand");
        assert_eq!(prog, "viewer");
        // "--opt=" is not empty so it remains; file arg is expanded normally.
        assert_eq!(args, vec!["--opt=", "/tmp/img.png"]);
    }

    #[test]
    fn expand_exec_template_converts_double_percent_to_literal() {
        let path = Path::new("/tmp/file");

        let (prog, args) =
            expand_exec_template("app --label=100%% %f", path).expect("should expand");
        assert_eq!(prog, "app");
        assert_eq!(args, vec!["--label=100%", "/tmp/file"]);
    }
}
