use super::metadata_preview::DocumentMetadata;
use std::{
    fs::File,
    io::Read,
    path::{Component, Path},
};
use zip::ZipArchive;

pub(super) const DOCUMENT_XML_ENTRY_LIMIT_BYTES: usize = 64 * 1024;

pub(super) fn extract_zip_document_metadata(
    path: &Path,
    extract: impl FnOnce(&mut ZipArchive<File>) -> DocumentMetadata,
) -> Option<DocumentMetadata> {
    let file = File::open(path).ok()?;
    let metadata = match ZipArchive::new(file) {
        Ok(mut archive) => extract(&mut archive),
        Err(_) => DocumentMetadata::default(),
    };
    Some(metadata)
}

pub(super) fn read_zip_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Option<String> {
    read_zip_entry_limited(archive, name, DOCUMENT_XML_ENTRY_LIMIT_BYTES)
}

pub(super) fn read_zip_entry_limited<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    limit_bytes: usize,
) -> Option<String> {
    let entry = archive.by_name(name).ok()?;
    let limit = (entry.size() as usize).min(limit_bytes);
    let mut bytes = Vec::with_capacity(limit);
    entry
        .take(limit_bytes as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    String::from_utf8(bytes).ok()
}

pub(super) fn read_zip_entry_bytes_limited<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    limit_bytes: usize,
) -> Option<Vec<u8>> {
    let entry = archive.by_name(name).ok()?;
    let limit = (entry.size() as usize).min(limit_bytes);
    let mut bytes = Vec::with_capacity(limit);
    entry
        .take(limit_bytes as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    (!bytes.is_empty()).then_some(bytes)
}

pub(super) fn resolve_zip_entry_path(base_path: &str, href: &str) -> String {
    let href = strip_fragment_identifier(href);
    let base = Path::new(base_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let joined = base.join(href);
    let mut parts = Vec::new();
    for component in joined.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::ParentDir => {
                parts.pop();
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }
    parts.join("/")
}

pub(super) fn strip_fragment_identifier(path: &str) -> &str {
    path.split_once('#').map(|(base, _)| base).unwrap_or(path)
}
