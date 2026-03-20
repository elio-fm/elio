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
    fn syntect_backends_fall_back_to_legacy_until_migration_enables_them() {
        let preview = render_code_preview(
            PreviewSpec::code("javascript", CodeBackend::Syntect, None),
            "const value = 1;\n",
            true,
            20,
            &|| false,
        );

        assert!(
            preview[0]
                .spans
                .iter()
                .any(|span| span.content.contains("const"))
        );
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
}
