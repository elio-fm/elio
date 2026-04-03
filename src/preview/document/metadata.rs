use crate::{
    file_info::DocumentFormat,
    preview::{PreviewContent, PreviewKind, appearance as theme},
};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

#[derive(Clone, Debug, Default)]
pub(super) struct DocumentMetadata {
    pub(super) variant: Option<String>,
    pub(super) title: Option<String>,
    pub(super) subject: Option<String>,
    pub(super) author: Option<String>,
    pub(super) modified_by: Option<String>,
    pub(super) application: Option<String>,
    pub(super) created: Option<String>,
    pub(super) modified: Option<String>,
    pub(super) stats: Vec<(String, String)>,
    pub(super) metadata: Vec<(String, String)>,
}

pub(super) fn render_document_preview(
    format: DocumentFormat,
    metadata: DocumentMetadata,
) -> PreviewContent {
    let mut lines = render_document_preview_lines(&metadata);
    if lines.is_empty() {
        lines.push(Line::from("No document metadata available"));
    }

    PreviewContent::new(PreviewKind::Document, lines).with_detail(format.detail_label())
}

pub(super) fn render_document_preview_lines(metadata: &DocumentMetadata) -> Vec<Line<'static>> {
    let palette = theme::palette();
    let mut lines = Vec::new();
    let document = [
        ("Variant", metadata.variant.as_deref()),
        ("Title", metadata.title.as_deref()),
        ("Subject", metadata.subject.as_deref()),
    ];
    let people = [
        ("Author", metadata.author.as_deref()),
        ("Modified By", metadata.modified_by.as_deref()),
        ("Application", metadata.application.as_deref()),
    ];
    let dates = [
        ("Created", metadata.created.as_deref()),
        ("Modified", metadata.modified.as_deref()),
    ];
    let label_width = document
        .iter()
        .chain(people.iter())
        .chain(dates.iter())
        .filter(|(_, value)| value.is_some())
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(0)
        .max(owned_section_label_width(&metadata.stats))
        .max(owned_section_label_width(&metadata.metadata))
        .max(6);

    push_section(&mut lines, "Document", &document, label_width, palette);
    push_section(&mut lines, "People", &people, label_width, palette);
    push_section(&mut lines, "Dates", &dates, label_width, palette);
    push_owned_section(&mut lines, "Stats", &metadata.stats, label_width, palette);
    push_owned_section(
        &mut lines,
        "Metadata",
        &metadata.metadata,
        label_width,
        palette,
    );
    lines
}

fn push_section(
    lines: &mut Vec<Line<'static>>,
    title: &str,
    fields: &[(&str, Option<&str>)],
    label_width: usize,
    palette: theme::Palette,
) {
    let visible_fields: Vec<_> = fields
        .iter()
        .filter_map(|(label, value)| value.map(|value| (*label, value)))
        .collect();
    if visible_fields.is_empty() {
        return;
    }
    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(section_line(title, palette));
    for (label, value) in visible_fields {
        lines.push(document_line(label, value, label_width, palette));
    }
}

fn push_owned_section(
    lines: &mut Vec<Line<'static>>,
    title: &str,
    fields: &[(String, String)],
    label_width: usize,
    palette: theme::Palette,
) {
    if fields.is_empty() {
        return;
    }
    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(section_line(title, palette));
    for (label, value) in fields {
        lines.push(document_line(label, value, label_width, palette));
    }
}

fn owned_section_label_width(fields: &[(String, String)]) -> usize {
    fields
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(0)
}

fn section_line(title: &str, palette: theme::Palette) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(palette.accent),
    ))
}

fn document_line(
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
