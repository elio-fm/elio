use super::super::{
    common::{local_name, read_zip_entry_limited, resolve_zip_entry_path, xml_attribute_value},
    metadata::{
        DocumentMetadata, render_document_field_lines, render_document_preview,
        render_document_preview_lines,
    },
};
use super::{
    EPUB_CONTENT_ENTRY_LIMIT_BYTES, EPUB_COVER_ENTRY_LIMIT_BYTES,
    EPUB_SECTION_IMAGE_ENTRY_LIMIT_BYTES, EPUB_SECTION_TEXT_LIMIT_CHARS, append_epub_text_fragment,
    assets::{ExtractedEpubAsset, extract_epub_asset, extract_epub_asset_descriptor},
    cache::load_epub_package,
    toc::epub_section_title_from_path,
};
use crate::{
    file_info::DocumentFormat,
    preview::{
        PreviewContent, PreviewKind, PreviewVisual, PreviewVisualKind, PreviewVisualLayout,
        render_reflowed_text_preview,
    },
};
use quick_xml::{Reader, events::Event};
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
    visual: Option<PreviewVisual>,
}

struct EpubSectionPreview {
    text: String,
    truncation_note: Option<String>,
    visual: Option<PreviewVisual>,
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

fn extract_epub_section_preview<R: Read + std::io::Seek>(
    source_path: &Path,
    archive: &mut ZipArchive<R>,
    section_path: &str,
) -> EpubSectionPreview {
    let Some(xml) = read_zip_entry_limited(archive, section_path, EPUB_CONTENT_ENTRY_LIMIT_BYTES)
    else {
        return EpubSectionPreview {
            text: String::new(),
            truncation_note: None,
            visual: None,
        };
    };
    let blocks = extract_xhtml_text_blocks(&xml);
    let visual = extract_xhtml_image_href(&xml).and_then(|href| {
        let asset_path = resolve_zip_entry_path(section_path, &href);
        extract_epub_asset(
            source_path,
            archive,
            &asset_path,
            EPUB_SECTION_IMAGE_ENTRY_LIMIT_BYTES,
        )
        .map(|asset| {
            build_preview_visual(
                PreviewVisualKind::PageImage,
                PreviewVisualLayout::FullHeight,
                asset,
            )
        })
    });
    if blocks.is_empty() {
        return EpubSectionPreview {
            text: String::new(),
            truncation_note: None,
            visual,
        };
    }

    let mut text = String::new();
    let mut truncated = false;
    for block in blocks {
        let remaining = EPUB_SECTION_TEXT_LIMIT_CHARS.saturating_sub(text.chars().count());
        if remaining == 0 {
            truncated = true;
            break;
        }

        let Some((clipped, was_truncated)) = clip_epub_block(&block, remaining) else {
            continue;
        };
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(&clipped);
        if was_truncated {
            truncated = true;
            break;
        }
    }

    EpubSectionPreview {
        text,
        truncation_note: truncated.then(epub_section_truncation_note),
        visual,
    }
}

fn build_preview_visual(
    kind: PreviewVisualKind,
    layout: PreviewVisualLayout,
    asset: ExtractedEpubAsset,
) -> PreviewVisual {
    PreviewVisual {
        kind,
        layout,
        path: asset.path,
        size: asset.size,
        modified: asset.modified,
    }
}

fn extract_xhtml_text_blocks(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut blocks = Vec::new();
    let mut current = String::new();
    let mut skip_depth = 0usize;
    let mut body_depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "body" {
                    body_depth += 1;
                    continue;
                }
                if epub_skip_tag(&tag) {
                    skip_depth += 1;
                    continue;
                }
                if body_depth > 0 && skip_depth == 0 && epub_block_tag(&tag) {
                    flush_epub_text_block(&mut blocks, &mut current);
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = local_name(event.name().as_ref());
                if body_depth > 0 && skip_depth == 0 && (epub_block_tag(&tag) || tag == "br") {
                    flush_epub_text_block(&mut blocks, &mut current);
                }
            }
            Ok(Event::Text(text)) => {
                if body_depth == 0 || skip_depth > 0 {
                    continue;
                }
                if let Ok(value) = text.decode() {
                    append_epub_text_fragment(&mut current, value.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if body_depth == 0 || skip_depth > 0 {
                    continue;
                }
                if let Ok(value) = text.decode() {
                    append_epub_text_fragment(&mut current, value.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "body" {
                    flush_epub_text_block(&mut blocks, &mut current);
                    body_depth = body_depth.saturating_sub(1);
                    continue;
                }
                if epub_skip_tag(&tag) && skip_depth > 0 {
                    skip_depth -= 1;
                    continue;
                }
                if body_depth > 0 && skip_depth == 0 && epub_block_tag(&tag) {
                    flush_epub_text_block(&mut blocks, &mut current);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    flush_epub_text_block(&mut blocks, &mut current);
    blocks
}

fn extract_xhtml_image_href(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut body_depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "body" {
                    body_depth += 1;
                    continue;
                }
                if body_depth == 0 {
                    continue;
                }
                if tag == "img" {
                    if let Some(src) = xml_attribute_value(&event, reader.decoder(), "src") {
                        return Some(src);
                    }
                } else if tag == "image" {
                    if let Some(href) = xml_attribute_value(&event, reader.decoder(), "href") {
                        return Some(href);
                    }
                } else if tag == "object"
                    && let Some(data) = xml_attribute_value(&event, reader.decoder(), "data")
                {
                    return Some(data);
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = local_name(event.name().as_ref());
                if body_depth == 0 {
                    continue;
                }
                if tag == "img" {
                    if let Some(src) = xml_attribute_value(&event, reader.decoder(), "src") {
                        return Some(src);
                    }
                } else if tag == "image" {
                    if let Some(href) = xml_attribute_value(&event, reader.decoder(), "href") {
                        return Some(href);
                    }
                } else if tag == "object"
                    && let Some(data) = xml_attribute_value(&event, reader.decoder(), "data")
                {
                    return Some(data);
                }
            }
            Ok(Event::End(event)) => {
                if local_name(event.name().as_ref()) == "body" {
                    body_depth = body_depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    None
}

fn epub_skip_tag(tag: &str) -> bool {
    matches!(tag, "head" | "script" | "style" | "svg" | "math")
}

fn epub_block_tag(tag: &str) -> bool {
    matches!(
        tag,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "caption"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "figcaption"
            | "footer"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "li"
            | "p"
            | "pre"
            | "section"
            | "td"
            | "th"
            | "tr"
    )
}

fn flush_epub_text_block(blocks: &mut Vec<String>, current: &mut String) {
    let text = current.trim();
    if !text.is_empty() {
        blocks.push(text.to_string());
    }
    current.clear();
}

fn clip_epub_block(block: &str, remaining: usize) -> Option<(String, bool)> {
    if remaining == 0 {
        return None;
    }
    let char_count = block.chars().count();
    if char_count <= remaining {
        return Some((block.to_string(), false));
    }
    let clipped = block
        .chars()
        .take(remaining.saturating_sub(1))
        .collect::<String>();
    let clipped = clipped.trim_end();
    (!clipped.is_empty()).then(|| (format!("{clipped}…"), true))
}

fn epub_section_truncation_note() -> String {
    format!(
        "section excerpt limited to {} KiB",
        EPUB_SECTION_TEXT_LIMIT_CHARS / 1024
    )
}
