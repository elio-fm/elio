use super::*;

#[test]
fn tokenizes_quoted_command() {
    assert_eq!(
        tokenize_command("open -a \"Preview App\" {path}"),
        vec!["open", "-a", "Preview App", "{path}"]
    );
}

#[test]
fn preserves_backslashes_that_are_not_command_escapes() {
    assert_eq!(
        tokenize_command(r#"echo C:\tmp\notes.txt"#),
        vec!["echo", r#"C:\tmp\notes.txt"#]
    );
}

#[test]
fn still_supports_escaped_unix_whitespace() {
    assert_eq!(
        tokenize_command(r#"my\ command --flag"#),
        vec!["my command", "--flag"]
    );
}

#[test]
fn tokenizes_quoted_windows_command_path() {
    assert_eq!(
        tokenize_command(r#""C:\Program Files\SumatraPDF\SumatraPDF.exe" -reuse-instance"#),
        vec![
            r#"C:\Program Files\SumatraPDF\SumatraPDF.exe"#,
            "-reuse-instance"
        ]
    );
}

#[test]
fn tokenizes_unquoted_windows_command_path_with_spaces() {
    assert_eq!(
        tokenize_command(r#"C:\Program Files\SumatraPDF\SumatraPDF.exe -reuse-instance"#),
        vec![
            r#"C:\Program Files\SumatraPDF\SumatraPDF.exe"#,
            "-reuse-instance"
        ]
    );
}

#[test]
fn tokenizes_windows_forward_slash_command_path_with_spaces() {
    assert_eq!(
        tokenize_command(r#"C:/Program Files/SumatraPDF/SumatraPDF.exe -reuse-instance"#),
        vec![
            "C:/Program Files/SumatraPDF/SumatraPDF.exe",
            "-reuse-instance"
        ]
    );
}

#[test]
fn expands_embedded_path_placeholder_once_per_path() {
    let paths = vec![PathBuf::from("a.md"), PathBuf::from("b.md")];
    assert_eq!(
        expand_args(&["--file={path}".to_string()], &paths),
        vec!["--file=a.md", "--file=b.md"]
    );
}

#[test]
fn editor_command_uses_editor_even_when_visual_is_set() {
    let template = command_template_with_env("$EDITOR --cmd {path}", |key| match key {
        "EDITOR" => Some(OsString::from("nvim --clean")),
        "VISUAL" => Some(OsString::from("hx")),
        _ => None,
    })
    .expect("$EDITOR should expand");

    assert_eq!(template.program, "nvim");
    assert_eq!(template.args, vec!["--clean", "--cmd", "{path}"]);
}

#[test]
fn visual_command_uses_visual_even_when_editor_is_set() {
    let template = command_template_with_env("$VISUAL", |key| match key {
        "EDITOR" => Some(OsString::from("nvim")),
        "VISUAL" => Some(OsString::from("code --wait")),
        _ => None,
    })
    .expect("$VISUAL should expand");

    assert_eq!(template.program, "code");
    assert_eq!(template.args, vec!["--wait"]);
}

#[test]
fn editor_command_does_not_fall_back_to_visual() {
    let error = command_template_with_env("$EDITOR", |key| match key {
        "VISUAL" => Some(OsString::from("hx")),
        _ => None,
    })
    .expect_err("$EDITOR should be required");

    assert_eq!(error, "$EDITOR is not set");
}

#[test]
fn appends_paths_to_matching_command_group() {
    let mut plans = Vec::new();
    push_command_plan(
        &mut plans,
        true,
        "hx".to_string(),
        vec!["--foo".to_string(), "a.md".to_string()],
        true,
    );
    push_command_plan(
        &mut plans,
        true,
        "hx".to_string(),
        vec!["--foo".to_string(), "b.md".to_string()],
        true,
    );
    assert_eq!(
        plans,
        vec![OpenPlan::Terminal {
            program: "hx".to_string(),
            args: vec!["--foo".to_string(), "a.md".to_string(), "b.md".to_string()],
        }]
    );
}
