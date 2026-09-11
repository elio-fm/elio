use super::super::{PreviewContent, PreviewKind, appearance as theme};
use crate::fs::Entry;
use image::ImageReader;
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(in crate::preview) fn build_image_preview(
    entry: &Entry,
    type_detail: Option<&'static str>,
) -> PreviewContent {
    let palette = theme::palette();
    let detail = type_detail.unwrap_or("Image");
    let byte_size = std::fs::metadata(&entry.path)
        .map(|metadata| metadata.len())
        .unwrap_or(entry.size);
    let mut fields = vec![("File Size", crate::fs::format_size(byte_size))];
    if let Ok((width_px, height_px)) = (|| {
        let reader = ImageReader::open(&entry.path)?;
        let reader = reader
            .with_guessed_format()
            .map_err(std::io::Error::other)?;
        reader.into_dimensions().map_err(std::io::Error::other)
    })() {
        fields.insert(0, ("Dimensions", format!("{width_px}x{height_px}")));
    }
    let label_width = fields
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(8);
    let mut lines = vec![preview_section_line("Details", palette)];
    for (label, value) in fields {
        lines.push(preview_field_line(label, &value, label_width, palette));
    }
    PreviewContent::new(PreviewKind::Image, lines).with_detail(detail)
}

fn preview_section_line(title: &str, palette: theme::Palette) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(palette.accent),
    ))
}

fn preview_field_line(
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

#[cfg(test)]
#[path = "tests/image_preview.rs"]
mod tests;
