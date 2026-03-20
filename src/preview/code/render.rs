use super::backends::{custom, legacy, plain, syntect};
use crate::file_info::{CodeBackend, PreviewSpec};
use ratatui::text::Line;

pub(crate) fn render_code_preview<F>(
    spec: PreviewSpec,
    text: &str,
    line_numbers: bool,
    line_limit: usize,
    canceled: &F,
) -> Vec<Line<'static>>
where
    F: Fn() -> bool,
{
    match spec.code_backend {
        CodeBackend::Plain => {
            plain::render_plain_code_preview(text, line_numbers, line_limit, canceled)
        }
        CodeBackend::Custom(kind) => {
            custom::render_custom_code_preview(kind, text, line_numbers, line_limit, canceled)
        }
        CodeBackend::Syntect => {
            render_syntect_with_fallback(spec, text, line_numbers, line_limit, canceled)
        }
    }
}

fn render_syntect_with_fallback<F>(
    spec: PreviewSpec,
    text: &str,
    line_numbers: bool,
    line_limit: usize,
    canceled: &F,
) -> Vec<Line<'static>>
where
    F: Fn() -> bool,
{
    let Some(code_syntax) = spec.code_syntax else {
        return plain::render_plain_code_preview(text, line_numbers, line_limit, canceled);
    };

    if !syntect::is_enabled(code_syntax) {
        return legacy::render_legacy_code_preview(
            text,
            spec.highlight_language(),
            line_numbers,
            line_limit,
            canceled,
        );
    }

    syntect::render_syntect_code_preview(code_syntax, text, line_numbers, line_limit, canceled)
        .unwrap_or_else(|_| {
            plain::render_plain_code_preview(text, line_numbers, line_limit, canceled)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_info::{
        CodeBackend, CustomCodeKind, HighlightLanguage, PreviewKind, PreviewSpec,
    };
    use ratatui::text::Line;
    use std::cell::Cell;

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    fn line_texts(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(line_text).collect()
    }

    #[test]
    fn enabled_javascript_preview_specs_use_syntect() {
        let preview = render_code_preview(
            PreviewSpec::code("javascript", CodeBackend::Syntect, None),
            "const value = 1;\n",
            true,
            20,
            &|| false,
        );
        let expected = syntect::render_syntect_code_preview(
            "javascript",
            "const value = 1;\n",
            true,
            20,
            &|| false,
        )
        .expect("javascript should render through syntect");

        assert_eq!(preview, expected);
    }

    #[test]
    fn enabled_typescript_family_uses_syntect_aliases() {
        let preview = render_code_preview(
            PreviewSpec::code("tsx", CodeBackend::Syntect, None),
            "export function App() { return <div>Hello</div>; }\n",
            true,
            20,
            &|| false,
        );
        let expected = syntect::render_syntect_code_preview(
            "tsx",
            "export function App() { return <div>Hello</div>; }\n",
            true,
            20,
            &|| false,
        )
        .expect("tsx should render through syntect");

        assert_eq!(preview, expected);
    }

    #[test]
    fn enabled_generic_source_preview_specs_use_syntect() {
        for (code_syntax, text) in [
            ("rust", "fn main() {}\n"),
            ("go", "package main\nfunc main() {}\n"),
            ("c", "#include <stdio.h>\nint main(void) { return 0; }\n"),
            (
                "cpp",
                "#include <iostream>\nint main() { std::cout << \"hi\"; }\n",
            ),
            (
                "java",
                "class Main {\n    public static void main(String[] args) {}\n}\n",
            ),
            ("php", "<?php echo \"hi\";\n"),
            ("python", "class Greeter:\n    pass\n"),
            ("ruby", "class Greeter\nend\n"),
            ("lua", "local name = \"elio\"\n"),
            ("make", "build:\n\tcc main.c\n"),
            ("bash", "export PATH=\"$HOME/bin:$PATH\"\n"),
            ("html", "<div class=\"app\">elio</div>\n"),
            ("xml", "<layout id=\"main\" />\n"),
            ("css", ".app { color: #fff; }\n"),
        ] {
            let preview = render_code_preview(
                PreviewSpec::code(code_syntax, CodeBackend::Syntect, None),
                text,
                true,
                20,
                &|| false,
            );
            let expected =
                syntect::render_syntect_code_preview(code_syntax, text, true, 20, &|| false)
                    .expect("{code_syntax} should render through syntect");

            assert_eq!(preview, expected, "expected {code_syntax} to use syntect");
        }
    }

    #[test]
    fn syntect_renderer_returns_error_for_unknown_syntax() {
        assert!(
            syntect::render_syntect_code_preview(
                "totally-unknown-syntax",
                "hello\n",
                true,
                20,
                &|| false,
            )
            .is_err()
        );
    }

    #[test]
    fn unsupported_syntect_specs_still_fall_back_to_legacy_rendering() {
        let preview = render_code_preview(
            PreviewSpec::code("cmake", CodeBackend::Syntect, None),
            "project(elio)\n",
            true,
            20,
            &|| false,
        );
        let expected = legacy::render_legacy_code_preview(
            "project(elio)\n",
            Some(HighlightLanguage::CMake),
            true,
            20,
            &|| false,
        );

        assert_eq!(preview, expected);
    }

    #[test]
    fn custom_preview_specs_use_custom_backend() {
        let preview = render_code_preview(
            PreviewSpec::code("jsonc", CodeBackend::Custom(CustomCodeKind::Jsonc), None),
            "{\n  // comment\n}\n",
            true,
            20,
            &|| false,
        );
        let expected = crate::preview::code::custom::render_custom_code_preview(
            CustomCodeKind::Jsonc,
            "{\n  // comment\n}\n",
            true,
            20,
            &|| false,
        );

        assert_eq!(preview, expected);
    }

    #[test]
    fn unknown_syntect_preview_specs_fall_back_to_plain_text() {
        let text = "first()\nsecond()\n";
        let preview = render_code_preview(
            PreviewSpec::code("totally-unknown-syntax", CodeBackend::Syntect, None),
            text,
            true,
            20,
            &|| false,
        );
        let expected = plain::render_plain_code_preview(text, true, 20, &|| false);

        assert_eq!(preview, expected);
        assert_eq!(
            line_texts(&preview),
            vec![" 1 first()".to_string(), " 2 second()".to_string()]
        );
    }

    #[test]
    fn missing_syntect_code_syntax_falls_back_to_plain_text() {
        let text = "plain text\nstill visible\n";
        let preview = render_code_preview(
            PreviewSpec {
                kind: PreviewKind::Source,
                language_hint: Some("unknown"),
                code_syntax: None,
                code_backend: CodeBackend::Syntect,
                structured_format: None,
                document_format: None,
            },
            text,
            true,
            20,
            &|| false,
        );
        let expected = plain::render_plain_code_preview(text, true, 20, &|| false);

        assert_eq!(preview, expected);
    }

    #[test]
    fn golden_code_previews_keep_expected_text_layout_across_backends() {
        let syntect_preview = render_code_preview(
            PreviewSpec::code("rust", CodeBackend::Syntect, None),
            "fn main() {\n    println!(\"hi\");\n}\n",
            true,
            20,
            &|| false,
        );
        assert_eq!(
            line_texts(&syntect_preview),
            vec![
                " 1 fn main() {".to_string(),
                " 2     println!(\"hi\");".to_string(),
                " 3 }".to_string(),
            ]
        );

        let custom_preview = render_code_preview(
            PreviewSpec::code("jsonc", CodeBackend::Custom(CustomCodeKind::Jsonc), None),
            "{\n  // keep me\n  \"name\": \"elio\"\n}\n",
            true,
            20,
            &|| false,
        );
        assert_eq!(
            line_texts(&custom_preview),
            vec![
                " 1 {".to_string(),
                " 2   // keep me".to_string(),
                " 3   \"name\": \"elio\"".to_string(),
                " 4 }".to_string(),
            ]
        );
    }

    #[test]
    fn renderer_respects_line_limit_before_backend_dispatch() {
        let syntect_preview = render_code_preview(
            PreviewSpec::code("rust", CodeBackend::Syntect, None),
            "fn one() {}\nfn two() {}\nfn three() {}\n",
            true,
            2,
            &|| false,
        );
        assert_eq!(
            line_texts(&syntect_preview),
            vec![" 1 fn one() {}".to_string(), " 2 fn two() {}".to_string()]
        );

        let custom_preview = render_code_preview(
            PreviewSpec::code("jsonc", CodeBackend::Custom(CustomCodeKind::Jsonc), None),
            "{\n  // first\n  // second\n}\n",
            true,
            2,
            &|| false,
        );
        assert_eq!(
            line_texts(&custom_preview),
            vec![" 1 {".to_string(), " 2   // first".to_string()]
        );
    }

    #[test]
    fn renderer_stops_on_cancellation_without_empty_placeholder() {
        let calls = Cell::new(0usize);
        let preview = render_code_preview(
            PreviewSpec::code("rust", CodeBackend::Syntect, None),
            "fn first() {}\nfn second() {}\nfn third() {}\n",
            true,
            20,
            &|| {
                let current = calls.get();
                calls.set(current + 1);
                current >= 1
            },
        );

        assert_eq!(line_texts(&preview), vec![" 1 fn first() {}".to_string()]);

        let canceled_immediately = render_code_preview(
            PreviewSpec::code("rust", CodeBackend::Syntect, None),
            "fn hidden() {}\n",
            true,
            20,
            &|| true,
        );
        assert!(canceled_immediately.is_empty());
    }
}
