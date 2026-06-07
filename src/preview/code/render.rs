use super::backends::{custom, plain, syntect};
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
        return plain::render_plain_code_preview(text, line_numbers, line_limit, canceled);
    }

    syntect::render_syntect_code_preview(code_syntax, text, line_numbers, line_limit, canceled)
        .unwrap_or_else(|_| {
            plain::render_plain_code_preview(text, line_numbers, line_limit, canceled)
        })
}

#[cfg(test)]
mod tests;
