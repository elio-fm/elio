use ratatui::text::Line;

pub(in crate::preview::code) fn render_plain_code_preview<F>(
    text: &str,
    line_numbers: bool,
    line_limit: usize,
    canceled: &F,
) -> Vec<Line<'static>>
where
    F: Fn() -> bool,
{
    crate::preview::highlighting::render_code_preview_with(
        text,
        None,
        line_numbers,
        line_limit,
        canceled,
    )
}
