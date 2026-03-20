use crate::file_info::CustomCodeKind;
use ratatui::text::Line;

pub(in crate::preview::code) fn render_custom_code_preview<F>(
    kind: CustomCodeKind,
    text: &str,
    line_numbers: bool,
    line_limit: usize,
    canceled: &F,
) -> Vec<Line<'static>>
where
    F: Fn() -> bool,
{
    crate::preview::code::custom::render_custom_code_preview(
        kind,
        text,
        line_numbers,
        line_limit,
        canceled,
    )
}
