use super::backends::{legacy, plain, syntect};
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
        CodeBackend::Custom(_) => legacy::render_legacy_code_preview(
            text,
            spec.highlight_language(),
            line_numbers,
            line_limit,
            canceled,
        ),
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
    use crate::file_info::{CodeBackend, PreviewSpec};

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

        assert!(
            preview[0]
                .spans
                .iter()
                .any(|span| span.content.contains("project"))
        );
    }
}
