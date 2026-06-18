mod backends;
mod cache;
mod extract;
mod metadata;
mod types;

use self::backends::has_rar_capable_extractor;
#[cfg(test)]
use self::backends::{
    parse_comic_archive_from_7z_output, parse_unrar_archive_comment, parse_zip_comic_archive,
    sniff_comic_archive_signature,
};
use self::cache::load_comic_archive;
use self::extract::extract_comic_archive_page_visual;
#[cfg(test)]
use self::metadata::parse_comic_book_info_comment;
use self::types::CachedComicArchive;
#[cfg(test)]
use self::types::{ComicArchiveBackend, ComicArchiveSignature};
use super::format::archive_default_label;
use super::*;
use std::path::Path;

pub(super) fn build_comic_archive_preview<F>(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
    page_index: usize,
    canceled: &F,
) -> Option<PreviewContent>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }

    let Some(comic) = load_comic_archive(path, canceled) else {
        if matches!(format, ArchiveFormat::ComicRar) {
            let detail = type_detail
                .unwrap_or(archive_default_label(format))
                .to_string();
            let note = if has_rar_capable_extractor() {
                "Unable to read RAR archive (file may be corrupted or unsupported)"
            } else {
                "RAR preview requires unrar or a 7z build with RAR support"
            };
            return Some(
                PreviewContent::new(PreviewKind::Comic, Vec::new())
                    .with_detail(detail)
                    .with_status_note(note),
            );
        }
        return None;
    };
    if comic.page_entries.is_empty() {
        return None;
    }

    let current_index = page_index.min(comic.page_entries.len().saturating_sub(1));
    let detail = type_detail
        .unwrap_or(archive_default_label(format))
        .to_string();
    let lines = comic_archive_details_lines(&comic);
    let mut preview = PreviewContent::new(PreviewKind::Comic, lines)
        .with_detail(detail)
        .with_navigation_position("Page", current_index, comic.page_entries.len(), None);

    if canceled() {
        return None;
    }

    if let Some(visual) = extract_comic_archive_page_visual(
        path,
        &comic,
        &comic.page_entries[current_index],
        canceled,
    ) {
        preview = preview.with_preview_visual(visual);
    } else {
        preview = preview.with_status_note("Unable to extract selected page");
    }

    Some(preview)
}

fn comic_archive_details_lines(comic: &CachedComicArchive) -> Vec<Line<'static>> {
    let info = comic.comic_info.as_ref();
    let derived = comic.derived_info.as_ref();
    if info.is_none() && derived.is_none() {
        return Vec::new();
    }
    let palette = theme::palette();
    let fields = vec![
        ("Title", info.and_then(|info| info.title.clone())),
        (
            "Series",
            info.and_then(|info| info.series.clone())
                .or_else(|| derived.and_then(|info| info.series.clone())),
        ),
        (
            "Number",
            info.and_then(|info| info.number.clone())
                .or_else(|| derived.and_then(|info| info.number.clone())),
        ),
        (
            "Volume",
            info.and_then(|info| info.volume.clone())
                .or_else(|| derived.and_then(|info| info.volume.clone())),
        ),
        (
            "Year",
            info.and_then(|info| info.year.clone())
                .or_else(|| derived.and_then(|info| info.year.clone())),
        ),
        (
            "Publisher",
            info.and_then(|info| info.publisher.clone())
                .or_else(|| derived.and_then(|info| info.publisher.clone())),
        ),
        ("Writer", info.and_then(|info| info.writer.clone())),
        ("Penciller", info.and_then(|info| info.penciller.clone())),
        ("Genre", info.and_then(|info| info.genre.clone())),
        ("Source", derived.and_then(|info| info.source.clone())),
        ("Chapters", derived.and_then(|info| info.chapters.clone())),
    ];
    let mut lines = Vec::new();
    push_preview_section(&mut lines, "Details", &fields, palette);
    lines
}

#[cfg(test)]
mod tests;
