use super::*;

#[test]
fn rendered_syntect_colors_only_use_elio_code_palette() {
    let allowed = palette_colors();
    let rendered = render_syntect_code_preview(
        "rust",
        "fn main() {\n    let answer = 42;\n    println!(\"hi\"); // note\n}\n",
        true,
        20,
        &|| false,
    )
    .expect("rust syntax should render through syntect");

    for line in &rendered {
        for span in &line.spans {
            if let Some(color) = span.style.fg {
                assert!(
                    allowed.contains(&color),
                    "found non-Elio syntect color {color:?} in span {:?}",
                    span.content
                );
            }
        }
    }
}

#[test]
fn rendered_syntect_tokens_map_to_elio_semantic_roles() {
    let palette = theme::code_preview_palette();
    let rust = render_syntect_code_preview(
        "rust",
        "fn main() {\n    let answer = 42;\n    println!(\"hi\"); // note\n}\n",
        true,
        20,
        &|| false,
    )
    .expect("rust syntax should render through syntect");
    assert_eq!(span_color(&rust[0], "fn"), Some(palette.keyword));
    assert_eq!(span_color(&rust[1], "42"), Some(palette.constant));
    assert!(
        rust[2]
            .spans
            .iter()
            .any(|span| span.style.fg == Some(palette.string)),
        "expected a string-colored span in {:?}",
        rust[2]
    );
    assert!(
        rust[2]
            .spans
            .iter()
            .any(|span| span.style.fg == Some(palette.comment)),
        "expected a comment-colored span in {:?}",
        rust[2]
    );

    let html =
        render_syntect_code_preview("html", "<div class=\"app\">elio</div>\n", true, 20, &|| {
            false
        })
        .expect("html syntax should render through syntect");
    assert_eq!(span_color(&html[0], "div"), Some(palette.tag));
    assert_eq!(span_color(&html[0], "class"), Some(palette.parameter));
}

#[test]
fn powershell_tokens_map_to_semantic_roles() {
    let palette = theme::code_preview_palette();
    let sample = "function Invoke-Greeting([string]$Name) {\n  if ($Name) { Write-Host \"Hello $Name\" }\n}\n";
    let rendered = render_syntect_code_preview("powershell", sample, true, 20, &|| false)
        .expect("powershell syntax should render through syntect");

    assert_eq!(span_color(&rendered[0], "function"), Some(palette.keyword));
    assert_eq!(
        span_color(&rendered[0], "Invoke-Greeting"),
        Some(palette.function)
    );
    assert_eq!(span_color(&rendered[0], "[string]"), Some(palette.r#type));
    assert_eq!(span_color(&rendered[0], "$Name"), Some(palette.parameter));
    assert_ne!(span_color(&rendered[1], "Write-Host"), Some(palette.fg));
    assert_eq!(span_color(&rendered[1], "\"Hello "), Some(palette.string));
    assert_eq!(span_color(&rendered[1], "$Name"), Some(palette.parameter));
}

#[test]
fn qml_tokens_map_to_semantic_roles() {
    let palette = theme::code_preview_palette();
    let sample = "import QtQuick\nItem {\n  id: root\n  required property bool active: true\n  Component.onCompleted: {\n    if (active) {\n      console.log(\"hello\")\n    }\n  }\n}\n";
    let rendered = render_syntect_code_preview("qml", sample, true, 20, &|| false)
        .expect("qml syntax should render through syntect");

    assert_eq!(span_color(&rendered[1], "Item"), Some(palette.r#type));
    assert_eq!(span_color(&rendered[3], "required"), Some(palette.keyword));
    assert_eq!(span_color(&rendered[3], "property"), Some(palette.keyword));
    assert_eq!(span_color(&rendered[3], "active"), Some(palette.parameter));
    assert_eq!(
        span_color(&rendered[4], "Component.onCompleted"),
        Some(palette.parameter)
    );
    assert_eq!(span_color(&rendered[5], "if"), Some(palette.keyword));
    assert_eq!(span_color(&rendered[6], "log"), Some(palette.function));
    assert_eq!(span_color(&rendered[6], "\"hello\""), Some(palette.string));
}

#[test]
fn sh_tokens_map_to_semantic_roles() {
    let palette = theme::code_preview_palette();
    let sample = "NAME=elio\nif [ -n \"$HOME\" ]; then\n  echo \"$NAME\"\nfi # done\n";
    let rendered = render_syntect_code_preview("sh", sample, true, 20, &|| false)
        .expect("sh syntax should render through syntect");
    let scopes = token_scopes("sh", sample);

    assert_ne!(
        span_color(&rendered[0], "NAME"),
        Some(palette.fg),
        "sh assignment fell back to fg; scopes: {scopes:#?}"
    );
    assert_ne!(
        span_color(&rendered[1], "if"),
        Some(palette.fg),
        "sh keyword fell back to fg; scopes: {scopes:#?}"
    );
    assert_ne!(
        span_color(&rendered[1], "$"),
        Some(palette.fg),
        "sh variable marker fell back to fg; scopes: {scopes:#?}"
    );
    assert_ne!(
        span_color(&rendered[1], "HOME"),
        Some(palette.fg),
        "sh variable fell back to fg; scopes: {scopes:#?}"
    );
    assert_ne!(
        span_color(&rendered[2], "echo"),
        Some(palette.fg),
        "sh builtin fell back to fg; scopes: {scopes:#?}"
    );
    assert_eq!(
        span_color(&rendered[3], "#"),
        Some(palette.comment),
        "sh comment marker did not map to comment color; scopes: {scopes:#?}"
    );
    assert_eq!(
        span_color(&rendered[3], " done"),
        Some(palette.comment),
        "sh comment did not map to comment color; scopes: {scopes:#?}"
    );
}

#[test]
fn diff_tokens_map_to_theme_palette_roles() {
    let tokens = token_scopes(
        "diff",
        "diff --git a/file b/file\n@@ -1 +1 @@\n-old\n+new\n",
    );

    let role_for = |needle: &str| {
        let (_token, scopes) = tokens
            .iter()
            .find(|(token, _scopes)| token.contains(needle))
            .unwrap_or_else(|| panic!("expected token containing {needle}"));
        let stack = ScopeStack::from_str(scopes).expect("scope stack should parse");
        semantic_role_for_token(needle, stack.as_slice())
    };

    assert_eq!(role_for("@@"), SemanticRole::Comment);
    assert_eq!(role_for("old"), SemanticRole::Invalid);
    assert_eq!(role_for("new"), SemanticRole::String);
}

#[test]
fn bash_tokens_map_to_semantic_roles() {
    let palette = theme::code_preview_palette();
    let sample = "NAME=elio\nif [ -n \"$HOME\" ]; then\n  echo \"$NAME\"\nfi # done\n";
    let rendered = render_syntect_code_preview("bash", sample, true, 20, &|| false)
        .expect("bash syntax should render through syntect");
    let scopes = token_scopes("bash", sample);

    assert_ne!(
        span_color(&rendered[0], "NAME"),
        Some(palette.fg),
        "bash assignment fell back to fg; scopes: {scopes:#?}"
    );
    assert_ne!(
        span_color(&rendered[1], "if"),
        Some(palette.fg),
        "bash keyword fell back to fg; scopes: {scopes:#?}"
    );
    assert_ne!(
        span_color(&rendered[1], "$"),
        Some(palette.fg),
        "bash variable marker fell back to fg; scopes: {scopes:#?}"
    );
    assert_ne!(
        span_color(&rendered[1], "HOME"),
        Some(palette.fg),
        "bash variable fell back to fg; scopes: {scopes:#?}"
    );
    assert_ne!(
        span_color(&rendered[2], "echo"),
        Some(palette.fg),
        "bash builtin fell back to fg; scopes: {scopes:#?}"
    );
    assert_eq!(
        span_color(&rendered[3], "#"),
        Some(palette.comment),
        "bash comment marker did not map to comment color; scopes: {scopes:#?}"
    );
    assert_eq!(
        span_color(&rendered[3], " done"),
        Some(palette.comment),
        "bash comment did not map to comment color; scopes: {scopes:#?}"
    );
}

#[test]
fn sh_plain_commands_and_functions_map_to_semantic_roles() {
    let palette = theme::code_preview_palette();
    let sample = "deploy() {\n  grep -q \"$HOME\" /etc/profile\n  my_tool --flag \"$NAME\"\n}\n";
    let rendered = render_syntect_code_preview("sh", sample, true, 20, &|| false)
        .expect("sh syntax should render through syntect");
    let scopes = token_scopes("sh", sample);

    assert_ne!(
        span_color(&rendered[0], "deploy"),
        Some(palette.fg),
        "sh function name fell back to fg; scopes: {scopes:#?}"
    );
    assert_ne!(
        span_color(&rendered[1], "grep"),
        Some(palette.fg),
        "sh command fell back to fg; scopes: {scopes:#?}"
    );
    assert_ne!(
        span_color(&rendered[1], "-q"),
        Some(palette.fg),
        "sh option fell back to fg; scopes: {scopes:#?}"
    );
    assert_ne!(
        span_color(&rendered[2], "my_tool"),
        Some(palette.fg),
        "sh custom command fell back to fg; scopes: {scopes:#?}"
    );
    assert_ne!(
        span_color(&rendered[2], "--flag"),
        Some(palette.fg),
        "sh long option fell back to fg; scopes: {scopes:#?}"
    );
}

#[test]
fn sh_common_builtins_and_redirections_map_to_semantic_roles() {
    let palette = theme::code_preview_palette();
    let sample = "#!/bin/sh\nset -e\ncd /tmp\ntrap 'cleanup' EXIT\nexport PATH=\"$HOME/bin:$PATH\"\nsource ./env.sh\nread -r NAME\nexec \"$NAME\" > /tmp/out.log\n";
    let rendered = render_syntect_code_preview("sh", sample, true, 20, &|| false)
        .expect("sh syntax should render through shell-aware renderer");

    assert_eq!(span_color(&rendered[0], "#!"), Some(palette.r#macro));
    assert_ne!(span_color(&rendered[1], "set"), Some(palette.fg));
    assert_ne!(span_color(&rendered[2], "cd"), Some(palette.fg));
    assert_ne!(span_color(&rendered[3], "trap"), Some(palette.fg));
    assert_ne!(span_color(&rendered[4], "export"), Some(palette.fg));
    assert_ne!(span_color(&rendered[5], "source"), Some(palette.fg));
    assert_ne!(span_color(&rendered[6], "read"), Some(palette.fg));
    assert_ne!(span_color(&rendered[7], "exec"), Some(palette.fg));
    assert_ne!(span_color(&rendered[7], ">"), Some(palette.fg));
}

#[test]
fn sh_heredoc_markers_and_pipeline_commands_map_to_semantic_roles() {
    let palette = theme::code_preview_palette();
    let sample = "cat <<EOF | grep -i \"$needle\"\n";
    let rendered = render_syntect_code_preview("sh", sample, true, 20, &|| false)
        .expect("sh syntax should render through shell-aware renderer");

    assert_eq!(span_color(&rendered[0], "cat"), Some(palette.function));
    assert_eq!(span_color(&rendered[0], "<<"), Some(palette.operator));
    assert_eq!(span_color(&rendered[0], "EOF"), Some(palette.parameter));
    assert_eq!(span_color(&rendered[0], "|"), Some(palette.operator));
    assert_eq!(span_color(&rendered[0], "grep"), Some(palette.function));
    assert_eq!(span_color(&rendered[0], "-i"), Some(palette.parameter));
    assert_eq!(span_color(&rendered[0], "$needle"), Some(palette.parameter));
}

#[test]
fn sh_export_like_builtins_keep_assignment_context_for_following_names() {
    let palette = theme::code_preview_palette();
    let sample = "export PATH=\"$HOME/bin\"\nreadonly NAME=elio\nlocal count=1\n";
    let rendered = render_syntect_code_preview("sh", sample, true, 20, &|| false)
        .expect("sh syntax should render through shell-aware renderer");

    assert_eq!(span_color(&rendered[0], "export"), Some(palette.function));
    assert_eq!(span_color(&rendered[0], "PATH"), Some(palette.parameter));
    assert_eq!(span_color(&rendered[0], "="), Some(palette.operator));
    assert_eq!(span_color(&rendered[1], "readonly"), Some(palette.function));
    assert_eq!(span_color(&rendered[1], "NAME"), Some(palette.parameter));
    assert_eq!(span_color(&rendered[1], "="), Some(palette.operator));
    assert_eq!(span_color(&rendered[2], "local"), Some(palette.function));
    assert_eq!(span_color(&rendered[2], "count"), Some(palette.parameter));
    assert_eq!(span_color(&rendered[2], "="), Some(palette.operator));
}

#[test]
fn semantic_role_classifier_covers_expected_scope_families() {
    let stack = ScopeStack::from_str("source.rust keyword.control.rust").unwrap();
    assert_eq!(
        semantic_role_for_token("if", stack.as_slice()),
        SemanticRole::Keyword
    );

    let stack =
        ScopeStack::from_str("text.html.basic meta.tag entity.other.attribute-name.html").unwrap();
    assert_eq!(
        semantic_role_for_token("class", stack.as_slice()),
        SemanticRole::Parameter
    );

    let stack = ScopeStack::from_str(
        "source.c meta.preprocessor.include entity.name.function.preprocessor",
    )
    .unwrap();
    assert_eq!(
        semantic_role_for_token("include", stack.as_slice()),
        SemanticRole::Macro
    );

    let stack = ScopeStack::from_str("source.shell.bash variable.other.readwrite.assignment.shell")
        .unwrap();
    assert_eq!(
        semantic_role_for_token("MAKE", stack.as_slice()),
        SemanticRole::Parameter
    );

    let stack = ScopeStack::from_str("source.js variable.other.readwrite.js").unwrap();
    assert_eq!(
        semantic_role_for_token("Greeter", stack.as_slice()),
        SemanticRole::Type
    );

    let stack = ScopeStack::from_str("source.shell.bash").unwrap();
    assert_eq!(
        semantic_role_for_token("then", stack.as_slice()),
        SemanticRole::Keyword
    );

    let stack = ScopeStack::from_str("source.shell.bash").unwrap();
    assert_eq!(
        semantic_role_for_token("printf", stack.as_slice()),
        SemanticRole::Function
    );

    let stack = ScopeStack::from_str("source.shell.bash").unwrap();
    assert_eq!(
        semantic_role_for_token("$HOME", stack.as_slice()),
        SemanticRole::Parameter
    );

    let stack = ScopeStack::from_str("source.shell.bash meta.function-call.shell").unwrap();
    assert_eq!(
        semantic_role_for_token("grep", stack.as_slice()),
        SemanticRole::Function
    );

    let stack =
        ScopeStack::from_str("source.shell.bash meta.function-call.arguments.shell").unwrap();
    assert_eq!(
        semantic_role_for_token("--flag", stack.as_slice()),
        SemanticRole::Parameter
    );
}

#[test]
fn sql_tokens_map_to_semantic_roles() {
    let palette = theme::code_preview_palette();
    let sample = concat!(
        "SELECT id, name FROM users WHERE id = 42; -- pick one\n",
        "CREATE TABLE items (id INTEGER PRIMARY KEY, label TEXT NOT NULL);\n",
        "INSERT INTO items VALUES (99, 'hello');\n",
    );
    let rendered = render_syntect_code_preview("sql", sample, true, 20, &|| false)
        .expect("sql syntax should render through syntect");
    let scopes = token_scopes("sql", sample);

    // DML keywords must be colored — not bleeding comment scope from line 1.
    assert_eq!(
        span_color(&rendered[0], "SELECT"),
        Some(palette.keyword),
        "SELECT should be keyword; scopes: {scopes:#?}"
    );
    assert_eq!(
        span_color(&rendered[0], "--"),
        Some(palette.comment),
        "-- should be comment color; scopes: {scopes:#?}"
    );
    // Line 2 must NOT be in comment scope despite the -- on line 1.
    assert_eq!(
        span_color(&rendered[1], "CREATE"),
        Some(palette.keyword),
        "CREATE on line 2 should be keyword, not comment; scopes: {scopes:#?}"
    );
    assert_eq!(
        span_color(&rendered[2], "INSERT INTO"),
        Some(palette.keyword),
        "INSERT INTO on line 3 should be keyword; scopes: {scopes:#?}"
    );
    // String literals must be string-colored.
    assert_eq!(
        span_color(&rendered[2], "hello"),
        Some(palette.string),
        "string literal should be string color; scopes: {scopes:#?}"
    );
    // Numeric constant (42 avoids collision with line numbers 1–3).
    assert_eq!(
        span_color(&rendered[0], "42"),
        Some(palette.constant),
        "numeric constant should be constant color; scopes: {scopes:#?}"
    );
    // Comparison operator must be operator color, not keyword (keyword.operator.*
    // prefix must not be swallowed by the broader keyword selector).
    assert_eq!(
        span_color(&rendered[0], "="),
        Some(palette.operator),
        "= should be operator color, not keyword; scopes: {scopes:#?}"
    );
}

#[test]
fn semantic_role_classifier_keeps_shell_heuristics_scoped_to_shell_sources() {
    let rust_stack = ScopeStack::from_str("source.rust meta.function-call.arguments.rust").unwrap();
    assert_eq!(
        semantic_role_for_token("--flag", rust_stack.as_slice()),
        SemanticRole::Fg
    );

    let shell_stack =
        ScopeStack::from_str("source.shell.bash meta.function-call.arguments.shell").unwrap();
    assert_eq!(
        semantic_role_for_token("--flag", shell_stack.as_slice()),
        SemanticRole::Parameter
    );
}
