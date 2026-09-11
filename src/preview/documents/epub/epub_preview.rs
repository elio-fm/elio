use super::super::metadata_preview::{
    DocumentMetadata, render_document_field_lines, render_document_preview,
    render_document_preview_lines,
};
use super::{
    EPUB_COVER_ENTRY_LIMIT_BYTES,
    book_sections::epub_section_title_from_path,
    image_extraction::extract_epub_asset_descriptor,
    package_cache::load_epub_package,
    section_preview::{build_preview_visual, extract_epub_section_preview},
};
use crate::{
    file_classification::DocumentFormat,
    preview::{
        PreviewContent, PreviewKind, PreviewVisualKind, PreviewVisualLayout,
        render_reflowed_text_preview,
    },
};
use ratatui::text::Line;
use std::{io::Read, path::Path};
use zip::ZipArchive;

struct EpubPreviewData {
    metadata: DocumentMetadata,
    section_index: usize,
    section_count: usize,
    section_title: Option<String>,
    section_text: String,
    truncation_note: Option<String>,
    visual: Option<crate::preview::PreviewVisual>,
}

pub(super) fn build_epub_preview(path: &Path, section_index: usize) -> Option<PreviewContent> {
    let file = std::fs::File::open(path).ok()?;
    let preview = match ZipArchive::new(file) {
        Ok(mut archive) => {
            render_epub_preview(extract_epub_preview_data(&mut archive, path, section_index))
        }
        Err(_) => render_document_preview(
            DocumentFormat::Epub,
            DocumentMetadata {
                variant: Some("EPUB package".to_string()),
                ..DocumentMetadata::default()
            },
        ),
    };
    Some(preview)
}

fn render_epub_preview(preview: EpubPreviewData) -> PreviewContent {
    let section_navigation_active = preview.section_count > 0;
    let lines = if preview.section_text.is_empty() {
        if preview.section_count == 0 {
            let mut lines = render_document_preview_lines(&preview.metadata);
            if lines.is_empty() {
                lines.push(Line::from("No readable content in this ebook"));
            }
            lines
        } else if preview
            .visual
            .as_ref()
            .is_some_and(|visual| visual.kind == PreviewVisualKind::PageImage)
        {
            epub_page_context_lines(&preview)
        } else {
            vec![Line::from("No readable content in this section")]
        }
    } else {
        render_reflowed_text_preview(&preview.section_text)
    };
    let detail = if section_navigation_active {
        DocumentFormat::Epub.detail_label().to_string()
    } else {
        preview
            .metadata
            .title
            .clone()
            .unwrap_or_else(|| DocumentFormat::Epub.detail_label().to_string())
    };
    let status_note = (!section_navigation_active).then(|| {
        let mut parts = vec![DocumentFormat::Epub.detail_label().to_string()];
        if let Some(author) = preview.metadata.author.as_deref() {
            parts.push(author.to_string());
        }
        parts.join("  •  ")
    });
    let mut content = PreviewContent::new(PreviewKind::Document, lines).with_detail(detail);
    if let Some(status_note) = status_note {
        content = content.with_status_note(status_note);
    }
    if preview.section_count > 0 {
        content = content.with_ebook_section(
            preview.section_index,
            preview.section_count,
            preview.section_title,
        );
    }
    if let Some(visual) = preview.visual {
        content = content.with_preview_visual(visual);
    }
    if let Some(note) = preview.truncation_note {
        content = content.with_truncation(note);
    }
    content
}

fn epub_page_context_lines(preview: &EpubPreviewData) -> Vec<Line<'static>> {
    let mut fields = Vec::new();
    let page = preview
        .section_title
        .as_deref()
        .map(epub_page_label)
        .unwrap_or_else(|| format!("{} of {}", preview.section_index + 1, preview.section_count));
    fields.push(("Page".to_string(), page));
    push_epub_context_field(&mut fields, "Title", preview.metadata.title.as_deref());
    push_epub_context_field(&mut fields, "Author", preview.metadata.author.as_deref());
    push_epub_context_field(&mut fields, "Subject", preview.metadata.subject.as_deref());
    push_epub_context_field(&mut fields, "Created", preview.metadata.created.as_deref());
    push_epub_context_field(
        &mut fields,
        "Modified",
        preview.metadata.modified.as_deref(),
    );
    fields.extend(preview.metadata.metadata.iter().cloned());
    fields.extend(preview.metadata.stats.iter().cloned());
    render_document_field_lines(&fields)
}

fn epub_page_label(title: &str) -> String {
    let title = title.trim();
    title
        .strip_prefix("Page ")
        .map(str::trim)
        .filter(|page| !page.is_empty())
        .unwrap_or(title)
        .to_string()
}

fn push_epub_context_field(fields: &mut Vec<(String, String)>, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        fields.push((label.to_string(), value.to_string()));
    }
}

fn extract_epub_preview_data<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    path: &Path,
    requested_section_index: usize,
) -> EpubPreviewData {
    let mut preview = EpubPreviewData {
        metadata: DocumentMetadata {
            variant: Some("EPUB package".to_string()),
            ..DocumentMetadata::default()
        },
        section_index: 0,
        section_count: 0,
        section_title: None,
        section_text: String::new(),
        truncation_note: None,
        visual: None,
    };
    let Some(package) = load_epub_package(archive, path) else {
        return preview;
    };
    let (section_index, section_count) = match package.sections.len() {
        0 => (0, 0),
        count => (requested_section_index.min(count.saturating_sub(1)), count),
    };

    preview.visual = package.cover_asset.as_ref().and_then(|asset| {
        extract_epub_asset_descriptor(path, archive, asset, EPUB_COVER_ENTRY_LIMIT_BYTES).map(
            |asset| {
                build_preview_visual(PreviewVisualKind::Cover, PreviewVisualLayout::Inline, asset)
            },
        )
    });
    preview.metadata = package.metadata.clone();
    preview.section_index = section_index;
    preview.section_count = section_count;

    if let Some(section) = package.sections.get(section_index) {
        let section_preview = extract_epub_section_preview(path, archive, &section.path);
        preview.section_text = section_preview.text;
        preview.section_title = section
            .title
            .clone()
            .or_else(|| epub_section_title_from_path(&section.path));
        preview.truncation_note = section_preview.truncation_note;
        if preview.section_text.is_empty()
            && let Some(visual) = section_preview.visual
        {
            preview.visual = Some(visual);
        }
    }
    preview
}
