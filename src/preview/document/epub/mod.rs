mod assets;
mod cache;
mod parse;
mod render;
mod toc;

use crate::preview::PreviewContent;
use std::{path::Path, time::SystemTime};

const EPUB_NAV_ENTRY_LIMIT_BYTES: usize = 96 * 1024;
const EPUB_CONTENT_ENTRY_LIMIT_BYTES: usize = 192 * 1024;
const EPUB_SECTION_TEXT_LIMIT_CHARS: usize = 32 * 1024;
const EPUB_COVER_ENTRY_LIMIT_BYTES: usize = 4 * 1024 * 1024;
const EPUB_SECTION_IMAGE_ENTRY_LIMIT_BYTES: usize = 32 * 1024 * 1024;
const EPUB_PACKAGE_CACHE_LIMIT: usize = 16;
const EPUB_ASSET_CACHE_VERSION: usize = 2;

pub(super) fn build_epub_preview(path: &Path, section_index: usize) -> Option<PreviewContent> {
    render::build_epub_preview(path, section_index)
}

#[cfg(test)]
pub(super) fn reset_epub_package_parse_count(path: &Path) {
    cache::reset_epub_package_parse_count(path);
}

#[cfg(test)]
pub(super) fn epub_package_parse_count(path: &Path) -> usize {
    cache::epub_package_parse_count(path)
}

#[cfg(test)]
pub(super) fn clear_epub_package_cache() {
    cache::clear_epub_package_cache();
}

fn append_epub_text_fragment(target: &mut String, fragment: &str) {
    let normalized = fragment.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return;
    }
    if !target.is_empty() && !target.chars().last().is_some_and(char::is_whitespace) {
        target.push(' ');
    }
    target.push_str(&normalized);
}

fn system_time_key(time: SystemTime) -> Option<(u64, u32)> {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| (duration.as_secs(), duration.subsec_nanos()))
}
