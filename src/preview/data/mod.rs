mod csv;
mod sqlite;

use super::*;
use crate::ui::theme;
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(super) use self::csv::build_csv_preview;
pub(super) use self::sqlite::build_sqlite_preview;

fn section_line(title: &str, palette: theme::Palette) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(palette.accent),
    ))
}

fn field_line(
    label: &str,
    value: &str,
    label_width: usize,
    palette: theme::Palette,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<width$} ", width = label_width + 1),
            Style::default().fg(palette.muted),
        ),
        Span::styled(value.to_string(), Style::default().fg(palette.text)),
    ])
}

fn push_data_section(
    lines: &mut Vec<Line<'static>>,
    title: &str,
    fields: &[(&str, Option<String>)],
    palette: theme::Palette,
) {
    let visible: Vec<(&str, &str)> = fields
        .iter()
        .filter_map(|(label, value)| value.as_deref().map(|v| (*label, v)))
        .collect();
    if visible.is_empty() {
        return;
    }
    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(section_line(title, palette));
    let label_width = visible.iter().map(|(l, _)| l.len()).max().unwrap_or(6);
    for (label, value) in &visible {
        lines.push(field_line(label, value, label_width, palette));
    }
}
