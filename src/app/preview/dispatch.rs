use super::*;
use crate::file_facts;

pub(super) fn should_build_preview_in_background(entry: &Entry) -> bool {
    let facts = file_facts::inspect_path(&entry.path, entry.kind);
    facts.builtin_class == FileClass::Archive || facts.preview.document_format.is_some()
}

pub(super) fn loading_preview_for(entry: &Entry) -> PreviewContent {
    let facts = file_facts::inspect_path(&entry.path, entry.kind);
    let detail = facts
        .specific_type_label
        .or_else(|| {
            facts
                .preview
                .document_format
                .map(|format| format.detail_label())
        })
        .unwrap_or("Preview")
        .to_string();
    let lines = if facts.builtin_class == FileClass::Archive {
        vec![
            Line::from("Loading preview"),
            Line::from("Inspecting archive contents in background"),
        ]
    } else if facts.preview.document_format.is_some() {
        vec![
            Line::from("Loading preview"),
            Line::from("Extracting document metadata in background"),
        ]
    } else {
        vec![
            Line::from("Loading preview"),
            Line::from("Preparing file preview in background"),
        ]
    };
    PreviewContent::new(PreviewKind::Unavailable, lines).with_detail(detail)
}

pub(super) fn build_preview(entry: &Entry) -> PreviewContent {
    if entry.is_dir() {
        return directory::build_directory_preview(entry);
    }

    let facts = file_facts::inspect_path(&entry.path, entry.kind);
    let preview_spec = facts.preview;
    let type_detail = facts.specific_type_label;
    if preview_spec.kind == file_facts::PreviewKind::Iso
        && let Some(preview) = container::build_iso_preview(&entry.path)
    {
        return preview;
    }
    if preview_spec.kind == file_facts::PreviewKind::Torrent
        && let Some(preview) = container::build_torrent_preview(&entry.path)
    {
        return preview;
    }
    if facts.builtin_class == FileClass::Archive
        && let Some(preview) = container::build_archive_preview(&entry.path, type_detail)
    {
        return preview;
    }
    if let Some(document_format) = preview_spec.document_format
        && let Some(preview) = document::build_document_preview(&entry.path, document_format)
    {
        return apply_type_detail(preview, type_detail);
    }

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
    let line_truncated = source_line_count > PREVIEW_RENDER_LINE_LIMIT;
    let mut preview_truncation_note = truncation_note(text_preview.bytes_truncated, line_truncated);

    if preview_spec.kind == file_facts::PreviewKind::Markdown {
        let preview = PreviewContent::new(
            PreviewKind::Markdown,
            markdown::render_markdown_preview(&text_preview.text),
        );
        return finalize_text_preview(
            preview,
            source_line_count,
            text_preview.bytes_truncated,
            preview_truncation_note,
        );
    }

    if preview_spec.kind == file_facts::PreviewKind::Source {
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
                    combine_preview_notes(
                        preview_truncation_note,
                        structured_preview.truncation_note.as_deref(),
                    ),
                );
            }
        }

        if preview_spec.force_fallback
            && let Some(fallback_syntax) = preview_spec.fallback_syntax
        {
            let preview = PreviewContent::new(
                PreviewKind::Code,
                fallback::render_fallback_code_preview(&text_preview.text, fallback_syntax, true),
            )
            .with_detail(fallback_syntax.detail_label());
            return finalize_text_preview(
                preview,
                source_line_count,
                text_preview.bytes_truncated,
                preview_truncation_note.clone(),
            );
        }

        if let Some(syntax) = syntax::find_code_syntax(&entry.path, preview_spec.syntax_hint) {
            let preview = PreviewContent::new(
                PreviewKind::Code,
                syntax::render_code_preview(
                    &entry.path,
                    &text_preview.text,
                    preview_spec.syntax_hint,
                    true,
                ),
            )
            .with_detail(syntax.name.clone());
            return finalize_text_preview(
                preview,
                source_line_count,
                text_preview.bytes_truncated,
                preview_truncation_note.clone(),
            );
        }

        if let Some(fallback_syntax) = preview_spec.fallback_syntax {
            let preview = PreviewContent::new(
                PreviewKind::Code,
                fallback::render_fallback_code_preview(&text_preview.text, fallback_syntax, true),
            )
            .with_detail(fallback_syntax.detail_label());
            return finalize_text_preview(
                preview,
                source_line_count,
                text_preview.bytes_truncated,
                preview_truncation_note,
            );
        }
    }

    let preview = PreviewContent::new(
        PreviewKind::Text,
        render_plain_text_preview(&text_preview.text),
    );
    finalize_text_preview(
        apply_type_detail(preview, type_detail),
        source_line_count,
        text_preview.bytes_truncated,
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
    let detail = io_error.map_or("Read error", support::describe_io_error);
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
