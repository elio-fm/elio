use super::{built_in_highlighting, plain_code, syntect_highlighting};
use crate::file_classification::{CodeBackend, PreviewSpec};
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
            plain_code::render_plain_code_preview(text, line_numbers, line_limit, canceled)
        }
        CodeBackend::Custom(kind) => built_in_highlighting::render_built_in_code_preview(
            kind,
            text,
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
        return plain_code::render_plain_code_preview(text, line_numbers, line_limit, canceled);
    };

    if !syntect_highlighting::is_enabled(code_syntax) {
        return plain_code::render_plain_code_preview(text, line_numbers, line_limit, canceled);
    }

    syntect_highlighting::render_syntect_code_preview(
        code_syntax,
        text,
        line_numbers,
        line_limit,
        canceled,
    )
    .unwrap_or_else(|_| {
        plain_code::render_plain_code_preview(text, line_numbers, line_limit, canceled)
    })
}

#[cfg(test)]
#[path = "tests/code_rendering.rs"]
mod tests;
