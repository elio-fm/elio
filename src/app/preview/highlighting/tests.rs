use super::*;

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
}

#[test]
fn highlighting_detail_label_is_plain() {
    assert_eq!(HighlightLanguage::Json.detail_label(), "JSON");
}

#[test]
fn markup_detail_label_is_plain() {
    assert_eq!(HighlightLanguage::Markup.detail_label(), "Markup");
}

#[test]
fn jsonc_highlighting_renderer_keeps_comments() {
    let lines = render_highlighted_code_preview_for_tests(
        "{\n  // comment\n  \"name\": \"elio\"\n}\n",
        HighlightLanguage::Jsonc,
        true,
    );

    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("// comment"))
    );
}

#[test]
fn jsonc_highlighting_renderer_keeps_multiline_block_comments() {
    let lines = render_highlighted_code_preview_for_tests(
        "{\n  /* first line\n     second line */\n  \"name\": \"elio\"\n}\n",
        HighlightLanguage::Jsonc,
        true,
    );

    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("/* first line"))
    );
    assert!(
        lines[2]
            .spans
            .iter()
            .any(|span| span.content.contains("second line */"))
    );
}

#[test]
fn jsonc_detail_label_is_plain() {
    assert_eq!(HighlightLanguage::Jsonc.detail_label(), "JSONC");
}

#[test]
fn desktop_highlighting_renderer_handles_unicode_values() {
    let lines = render_highlighted_code_preview_for_tests(
        "[Desktop Entry]\nName=エリオ\nName[ja]=日本語アプリ\n",
        HighlightLanguage::DesktopEntry,
        true,
    );

    assert!(
        lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("日本語アプリ"))
    );
}

#[test]
fn log_highlighting_renderer_highlights_levels_and_fields() {
    let lines = render_highlighted_code_preview_for_tests(
        "2026-03-10T12:00:00Z ERROR request_id=42 path=/login failed\n",
        HighlightLanguage::Log,
        true,
    );

    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("ERROR"))
    );
    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("request_id"))
    );
}

#[test]
fn markup_highlighting_renderer_highlights_tags_attributes_and_comments() {
    let lines = render_highlighted_code_preview_for_tests(
        "<!-- note -->\n<div class=\"app\" data-id=\"42\">elio</div>\n",
        HighlightLanguage::Markup,
        true,
    );

    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("<!-- note -->"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("div"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("class"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("\"app\""))
    );
}

#[test]
fn markup_highlighting_renderer_keeps_multiline_comments() {
    let lines = render_highlighted_code_preview_for_tests(
        "<!-- first line\nsecond line -->\n<section />\n",
        HighlightLanguage::Markup,
        true,
    );

    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("<!-- first line"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("second line -->"))
    );
}

#[test]
fn css_highlighting_renderer_highlights_properties_and_values() {
    let lines = render_highlighted_code_preview_for_tests(
        ".app {\n  color: #fff;\n  margin: 12px;\n}\n",
        HighlightLanguage::Css,
        true,
    );

    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("color"))
    );
    assert!(
        lines[2]
            .spans
            .iter()
            .any(|span| span.content.contains("12px"))
    );
}

#[test]
fn c_like_highlighting_renderer_highlights_directives_comments_and_calls() {
    let lines = render_highlighted_code_preview_for_tests(
        "#include <stdio.h>\nint main(void) {\n  printf(\"hi\"); /* note */\n}\n",
        HighlightLanguage::CLike,
        true,
    );

    assert!(lines[0].spans.iter().any(|span| span.content.contains("#")));
    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("include"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("main"))
    );
    assert!(
        lines[2]
            .spans
            .iter()
            .any(|span| span.content.contains("/* note */"))
    );
}

#[test]
fn c_like_highlighting_renderer_handles_unicode_without_panicking() {
    let lines = render_highlighted_code_preview_for_tests(
        "int main(void) {\n  printf(\"hola 👋\"); // áéíóú\n}\n",
        HighlightLanguage::CLike,
        true,
    );

    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("👋"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("// áéíóú"))
    );
}

#[test]
fn make_highlighting_renderer_highlights_rules_variables_and_recipes() {
    let lines = render_highlighted_code_preview_for_tests(
        "CC := clang\n.PHONY: build\nbuild: main.o util.o\n\t$(CC) -o app main.o util.o\n",
        HighlightLanguage::Make,
        true,
    );

    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("CC"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains(".PHONY"))
    );
    assert!(
        lines[2]
            .spans
            .iter()
            .any(|span| span.content.contains("build"))
    );
    assert!(
        lines[3]
            .spans
            .iter()
            .any(|span| span.content.contains("$(CC)"))
    );
}

#[test]
fn nix_highlighting_renderer_highlights_keywords_strings_and_comments() {
    let lines = render_highlighted_code_preview_for_tests(
        "let\n  name = \"elio\"; # note\nin name\n",
        HighlightLanguage::Nix,
        true,
    );

    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("let"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("\"elio\""))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("# note"))
    );
}

#[test]
fn nix_highlighting_renderer_handles_unicode_without_panicking() {
    let lines = render_highlighted_code_preview_for_tests(
        "let\n  name = \"hóla 👋\"; # áéíóú\nin name\n",
        HighlightLanguage::Nix,
        true,
    );

    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("👋"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("# áéíóú"))
    );
}

#[test]
fn cmake_highlighting_renderer_highlights_commands_variables_and_comments() {
    let lines = render_highlighted_code_preview_for_tests(
        "project(elio)\nset(NAME elio)\nmessage(STATUS \"hi ${NAME}\") # note\n",
        HighlightLanguage::CMake,
        true,
    );

    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("project"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("NAME"))
    );
    assert!(
        lines[2]
            .spans
            .iter()
            .any(|span| span.content.contains("${NAME}"))
    );
    assert!(
        lines[2]
            .spans
            .iter()
            .any(|span| span.content.contains("# note"))
    );
}

#[test]
fn cmake_highlighting_renderer_handles_unicode_without_panicking() {
    let lines = render_highlighted_code_preview_for_tests(
        "project(elio)\nmessage(STATUS \"hóla 👋\") # áéíóú\n",
        HighlightLanguage::CMake,
        true,
    );

    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("👋"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("# áéíóú"))
    );
}

#[test]
fn python_highlighting_renderer_highlights_decorators_docstrings_and_comments() {
    let lines = render_highlighted_code_preview_for_tests(
        "@app.get(\"/status\")\nasync def greet(name: str) -> str:\n    \"\"\"Return a greeting.\"\"\"\n    return f\"hi {name}\"  # note\n",
        HighlightLanguage::Python,
        true,
    );

    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("@app.get"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("async"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("greet"))
    );
    assert!(
        lines[2]
            .spans
            .iter()
            .any(|span| span.content.contains("\"\"\"Return a greeting.\"\"\""))
    );
    assert!(
        lines[3]
            .spans
            .iter()
            .any(|span| span.content.contains("# note"))
    );
}

#[test]
fn python_highlighting_renderer_handles_unicode_identifiers_without_panicking() {
    let lines = render_highlighted_code_preview_for_tests(
        "def saludar(nombre):\n    mensaje = f\"hola, {nombre} 👋\"\n    print(mensaje)  # áéíóú\n",
        HighlightLanguage::Python,
        true,
    );

    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("👋"))
    );
    assert!(
        lines[2]
            .spans
            .iter()
            .any(|span| span.content.contains("# áéíóú"))
    );
}

#[test]
fn js_like_highlighting_renderer_handles_unicode_without_panicking() {
    let lines = render_highlighted_code_preview_for_tests(
        "const saludo = \"hóla 👋\";\nconsole.log(saludo); // áéíóú\n",
        HighlightLanguage::JsLike,
        true,
    );

    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("👋"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("// áéíóú"))
    );
}

#[test]
fn json_highlighting_renderer_handles_unicode_without_panicking() {
    let lines = render_highlighted_code_preview_for_tests(
        "{ \"message\": \"hóla 👋\", \"note\": \"áéíóú\" }\n",
        HighlightLanguage::Json,
        true,
    );

    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("👋"))
    );
    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("áéíóú"))
    );
}

#[test]
fn shell_highlighting_renderer_highlights_assignments_keywords_and_expansions() {
    let lines = render_highlighted_code_preview_for_tests(
        "#!/usr/bin/env bash\nNAME=elio\nif [ -n \"$NAME\" ]; then\n  printf '%s' \"$(whoami)\"\nfi # done\n",
        HighlightLanguage::Shell,
        true,
    );
    let line_texts: Vec<_> = lines.iter().map(line_text).collect();

    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("#!"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("NAME"))
    );
    assert!(
        lines[2]
            .spans
            .iter()
            .any(|span| span.content.contains("if"))
    );
    assert!(
        lines[2]
            .spans
            .iter()
            .any(|span| span.content.contains("$NAME"))
    );
    assert!(line_texts[3].contains("$(whoami)"));
    assert!(
        lines[4]
            .spans
            .iter()
            .any(|span| span.content.contains("# done"))
    );
}

#[test]
fn shell_highlighting_renderer_keeps_env_prefix_commands_and_function_defs_readable() {
    let lines = render_highlighted_code_preview_for_tests(
        "DEBUG=1 PATH=\"$HOME/bin:$PATH\" env printf '%s\\n' \"$PATH\"\nhello() {\n  return 0\n}\n",
        HighlightLanguage::Shell,
        true,
    );

    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("DEBUG"))
    );
    assert!(
        lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("printf"))
    );
    assert!(
        lines[1]
            .spans
            .iter()
            .any(|span| span.content.contains("hello"))
    );
    assert!(
        lines[2]
            .spans
            .iter()
            .any(|span| span.content.contains("return"))
    );
}

#[test]
fn shell_highlighting_renderer_preserves_heredoc_blocks() {
    let lines = render_highlighted_code_preview_for_tests(
        "cat <<'EOF'\nhello $USER\nEOF\n",
        HighlightLanguage::Shell,
        true,
    );
    let line_texts: Vec<_> = lines.iter().map(line_text).collect();

    assert!(line_texts[0].contains("<<'EOF'"));
    assert!(line_texts[1].contains("hello $USER"));
    assert!(line_texts[2].contains("EOF"));
}
