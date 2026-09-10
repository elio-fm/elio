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

    let (prog, args) = expand_exec_template("app --label=100%% %f", path).expect("should expand");
    assert_eq!(prog, "app");
    assert_eq!(args, vec!["--label=100%", "/tmp/file"]);
}
