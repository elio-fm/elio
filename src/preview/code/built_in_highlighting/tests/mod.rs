use super::preview_rendering::render_built_in_code_preview;
use crate::{file_classification::CustomCodeKind, preview::appearance as theme};
use ratatui::text::Line;

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
}

fn assert_span_color(line: &Line<'_>, token: &str, expected: ratatui::style::Color) {
    assert!(
        line.spans
            .iter()
            .any(|span| span.content.contains(token) && span.style.fg == Some(expected)),
        "expected token {token:?} with color {expected:?} in line {:?}",
        line_text(line)
    );
}

mod directive_configs;
mod ini_files;
mod log_files;
mod structured_data;
