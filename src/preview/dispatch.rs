use super::{appearance as theme, *};
use crate::core::{Entry, FileClass};
use crate::file_info;
use crate::fs as browser_support;
use image::ImageReader;
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(crate) fn should_build_preview_in_background(entry: &Entry) -> bool {
    // Any selected file preview may touch the filesystem or decode enough content to
    // stall the UI on slow or remote storage. Keep the selection path strictly async.
    let _ = entry;
    true
}

pub(crate) fn preview_work_class(
    entry: &Entry,
    options: &PreviewRequestOptions,
) -> PreviewWorkClass {
    let facts = file_info::inspect_entry_cached(entry);
    if options.comic_page_index().is_some()
        || options.epub_section_index().is_some()
        || facts.builtin_class == FileClass::Audio
        || facts.builtin_class == FileClass::Archive
        || facts.builtin_class == FileClass::Video
        || facts.preview.kind == file_info::PreviewKind::Iso
        || facts.preview.kind == file_info::PreviewKind::Torrent
        || facts.preview.kind == file_info::PreviewKind::Sqlite  // SqliteCandidate stays Light
        || facts.preview.document_format.is_some()
    {
        PreviewWorkClass::Heavy
    } else {
        PreviewWorkClass::Light
    }
}

pub(crate) fn loading_preview_for(
    entry: &Entry,
    options: &PreviewRequestOptions,
) -> PreviewContent {
    if entry.is_dir() {
        return PreviewContent::new(PreviewKind::Directory, Vec::new()).with_detail("Loading");
    }
    let facts = file_info::inspect_entry_cached(entry);
    let detail = facts
        .specific_type_label
        .or_else(|| {
            facts
                .preview
                .document_format
                .map(|format| format.detail_label())
        })
        .or((facts.builtin_class == FileClass::Audio).then_some("Audio"))
        .or((facts.builtin_class == FileClass::Video).then_some("Video"))
        .unwrap_or("Preview")
        .to_string();
    let is_comic_page_preview = matches!(
        (facts.specific_type_label, options.comic_page_index()),
        (Some("Comic ZIP archive" | "Comic RAR archive"), Some(_))
    );
    let is_epub_section_preview = matches!(
        (facts.preview.document_format, options.epub_section_index()),
        (Some(file_info::DocumentFormat::Epub), Some(_))
    );
    let kind = if is_comic_page_preview {
        PreviewKind::Comic
    } else if is_epub_section_preview {
        PreviewKind::Document
    } else {
        loading_preview_kind(&facts)
    };
    let lines = if is_comic_page_preview
        || is_epub_section_preview
        || facts.builtin_class == FileClass::Audio
    {
        Vec::new()
    } else if facts.builtin_class == FileClass::Archive {
        vec![
            Line::from("Loading preview"),
            Line::from("Inspecting archive contents in background"),
        ]
    } else if facts.preview.document_format.is_some() {
        vec![
            Line::from("Loading preview"),
            Line::from("Extracting document metadata in background"),
        ]
    } else if facts.builtin_class == FileClass::Video {
        Vec::new()
    } else {
        vec![
            Line::from("Loading preview"),
            Line::from("Preparing file preview in background"),
        ]
    };
    PreviewContent::new(kind, lines).with_detail(detail)
}

fn loading_preview_kind(facts: &file_info::FileFacts) -> PreviewKind {
    if facts.builtin_class == FileClass::Archive {
        return PreviewKind::Archive;
    }
    if facts.preview.document_format.is_some() {
        return PreviewKind::Document;
    }
    if facts.builtin_class == FileClass::Audio {
        return PreviewKind::Audio;
    }
    if facts.builtin_class == FileClass::Image
        && facts.preview.kind != file_info::PreviewKind::Source
    {
        return PreviewKind::Image;
    }
    if facts.builtin_class == FileClass::Video {
        return PreviewKind::Video;
    }

    match facts.preview.kind {
        file_info::PreviewKind::Markdown => PreviewKind::Markdown,
        file_info::PreviewKind::Source => PreviewKind::Code,
        file_info::PreviewKind::PlainText | file_info::PreviewKind::Torrent => PreviewKind::Text,
        file_info::PreviewKind::Iso => PreviewKind::Archive,
        file_info::PreviewKind::Sqlite
        | file_info::PreviewKind::SqliteCandidate
        | file_info::PreviewKind::Csv => PreviewKind::Data,
    }
}

#[cfg(test)]
pub(crate) fn build_preview(entry: &Entry) -> PreviewContent {
    build_preview_with_options(entry, &PreviewRequestOptions::Default)
}

#[cfg(test)]
pub(crate) fn build_preview_with_options(
    entry: &Entry,
    options: &PreviewRequestOptions,
) -> PreviewContent {
    build_preview_with_options_and_code_line_limit(
        entry,
        options,
        default_code_preview_line_limit(),
        default_code_preview_line_limit(),
        false,
        false,
        &|| false,
    )
}

pub(crate) fn build_preview_with_options_and_code_line_limit<F>(
    entry: &Entry,
    options: &PreviewRequestOptions,
    code_line_limit: usize,
    code_render_limit: usize,
    ffprobe_available: bool,
    ffmpeg_available: bool,
    canceled: &F,
) -> PreviewContent
where
    F: Fn() -> bool,
{
    if entry.is_dir() {
        return directory::build_directory_preview(entry);
    }

    let facts = file_info::inspect_entry_cached(entry);
    let preview_spec = facts.preview;
    let type_detail = facts.specific_type_label;
    if preview_spec.kind == file_info::PreviewKind::Iso
        && let Some(preview) = container::build_iso_preview(&entry.path)
    {
        return preview;
    }
    if preview_spec.kind == file_info::PreviewKind::Torrent
        && let Some(preview) = container::build_torrent_preview(&entry.path)
    {
        return preview;
    }
    if facts.builtin_class == FileClass::Archive {
        if let Some(preview) = container::build_archive_preview(
            &entry.path,
            type_detail,
            options.comic_page_index(),
            canceled,
        ) {
            return preview;
        }
        if canceled() {
            return loading_preview_for(entry, options);
        }
    }
    if let Some(document_format) = preview_spec.document_format
        && let Some(preview) = document::build_document_preview(
            &entry.path,
            document_format,
            options.epub_section_index(),
        )
    {
        return apply_type_detail(preview, type_detail);
    }
    if facts.builtin_class == FileClass::Image
        && preview_spec.kind != file_info::PreviewKind::Source
    {
        return image_metadata_preview(entry, type_detail);
    }
    if facts.builtin_class == FileClass::Audio {
        return audio::build_audio_preview(
            entry,
            type_detail,
            ffprobe_available,
            ffmpeg_available,
            canceled,
        );
    }
    if facts.builtin_class == FileClass::Video {
        return video::build_video_preview(
            entry,
            type_detail,
            ffprobe_available,
            ffmpeg_available,
            canceled,
        );
    }

    if matches!(
        preview_spec.kind,
        file_info::PreviewKind::Sqlite | file_info::PreviewKind::SqliteCandidate
    ) && let Some(preview) = data::build_sqlite_preview(&entry.path)
    {
        return apply_type_detail(preview, type_detail);
    }
    // Not a SQLite file (e.g. a .db with different content) — fall through.

    let text_preview = match read_text_preview(&entry.path) {
        Ok(Some(text)) => text,
        Ok(None) => {
            if let Some(preview) = binary::build_binary_preview(&entry.path, type_detail) {
                return preview;
            }
            return apply_type_detail(binary_preview(), type_detail);
        }
        Err(error) => {
            return apply_type_detail(unavailable_file_preview(&error), type_detail);
        }
    };
    let source_line_count = count_source_lines(&text_preview.text);
    let effective_code_line_limit = clamp_code_preview_line_limit(code_line_limit);
    let line_truncated = source_line_count > PREVIEW_RENDER_LINE_LIMIT;
    let mut preview_truncation_note = truncation_note(text_preview.bytes_truncated, line_truncated);

    if preview_spec.kind == file_info::PreviewKind::Csv {
        let is_tsv = std::path::Path::new(&entry.name)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("tsv"));
        return data::build_csv_preview(
            &text_preview.text,
            is_tsv,
            type_detail,
            text_preview.bytes_truncated,
        );
    }

    if preview_spec.kind == file_info::PreviewKind::Markdown {
        let preview = PreviewContent::new(
            PreviewKind::Markdown,
            markdown::render_markdown_preview(&text_preview.text),
        );
        return finalize_text_preview(
            apply_type_detail(preview, type_detail),
            source_line_count,
            text_preview.bytes_truncated,
            line_truncated,
            preview_truncation_note,
        );
    }

    if preview_spec.kind == file_info::PreviewKind::Source {
        if let Some(structured_format) = preview_spec.structured_format {
            let structured_attempt = structured::render_structured_preview(
                &text_preview.text,
                structured_format,
                text_preview.bytes_truncated,
            );
            preview_truncation_note =
                combine_preview_notes(preview_truncation_note, structured_attempt.note.as_deref());

            if let Some(structured_preview) = structured_attempt.preview {
                let preview = PreviewContent::new(PreviewKind::Code, structured_preview.lines)
                    .with_detail(structured_preview.detail);
                return finalize_text_preview(
                    preview,
                    source_line_count,
                    false,
                    line_truncated,
                    combine_preview_notes(
                        preview_truncation_note,
                        structured_preview.truncation_note.as_deref(),
                    ),
                );
            }
        }

        // Apply code_render_limit: clamp to min(code_render_limit, effective_code_line_limit).
        let actual_render_limit = code_render_limit.min(effective_code_line_limit);
        let code_line_truncated = source_line_count > actual_render_limit;
        let code_truncation_note = truncation_note_with_line_limit(
            text_preview.bytes_truncated,
            code_line_truncated,
            actual_render_limit,
        );
        preview_truncation_note =
            combine_preview_notes(preview_truncation_note, code_truncation_note.as_deref());
        let mut preview = PreviewContent::new(
            PreviewKind::Code,
            code::render_code_preview(
                preview_spec,
                &text_preview.text,
                true,
                actual_render_limit,
                canceled,
            ),
        );
        if let Some(detail) = source_preview_detail(type_detail, preview_spec) {
            preview = preview.with_detail(detail);
        }
        // Set incremental_render_limit when more source lines exist than were rendered.
        if source_line_count > actual_render_limit {
            preview.incremental_render_limit = Some(actual_render_limit);
        }
        return finalize_text_preview_with_line_limit(
            preview,
            source_line_count,
            text_preview.bytes_truncated,
            code_line_truncated,
            preview_truncation_note,
            actual_render_limit,
        );
    }

    if facts.builtin_class == FileClass::License {
        let preview = PreviewContent::new(
            PreviewKind::Text,
            render_reflowed_text_preview(&text_preview.text),
        );
        return finalize_text_preview(
            apply_type_detail(preview, type_detail),
            source_line_count,
            text_preview.bytes_truncated,
            false,
            truncation_note(text_preview.bytes_truncated, false),
        );
    }

    let preview = PreviewContent::new(
        PreviewKind::Text,
        render_plain_text_preview(&text_preview.text),
    );
    finalize_text_preview(
        apply_type_detail(preview, type_detail),
        source_line_count,
        text_preview.bytes_truncated,
        line_truncated,
        preview_truncation_note,
    )
}

fn apply_type_detail(
    mut preview: PreviewContent,
    type_detail: Option<&'static str>,
) -> PreviewContent {
    if let Some(detail) = type_detail
        && matches!(
            preview.detail.as_deref(),
            None | Some("Binary file") | Some("Read error")
        )
    {
        preview.detail = Some(detail.to_string());
    }
    preview
}

fn source_preview_detail(
    type_detail: Option<&'static str>,
    preview_spec: file_info::PreviewSpec,
) -> Option<String> {
    type_detail
        .map(ToString::to_string)
        .or_else(|| preview_spec.language_hint.map(display_language_hint))
}

fn display_language_hint(language_hint: &str) -> String {
    super::code::registry::display_label_for_code_syntax(language_hint)
        .map(str::to_string)
        .unwrap_or_else(|| {
            let mut chars = language_hint.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
}

fn binary_preview() -> PreviewContent {
    super::status_preview(
        PreviewKind::Binary,
        "Binary file",
        [
            Line::from("No text preview available"),
            Line::from("Binary or unsupported file"),
        ],
    )
}

fn image_metadata_preview(entry: &Entry, type_detail: Option<&'static str>) -> PreviewContent {
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

fn unavailable_preview(detail: &str, message: &str) -> PreviewContent {
    super::status_preview(
        PreviewKind::Unavailable,
        detail,
        [
            Line::from("Preview unavailable"),
            Line::from(message.to_string()),
        ],
    )
}

fn unavailable_file_preview(error: &anyhow::Error) -> PreviewContent {
    let io_error = error.downcast_ref::<std::io::Error>();
    let detail = io_error.map_or("Read error", browser_support::describe_io_error);
    let message = match io_error.map(std::io::Error::kind) {
        Some(std::io::ErrorKind::PermissionDenied) => {
            "You do not have permission to read this file"
        }
        Some(std::io::ErrorKind::NotFound) => "This file is no longer available",
        Some(std::io::ErrorKind::Unsupported) => "This location is not supported",
        _ => "The file could not be read",
    };
    unavailable_preview(detail, message)
}
