pub(super) mod doc;
pub(super) mod kindle;
pub(super) mod ooxml;
pub(super) mod open_document;
pub(super) mod pages;
pub(super) mod pdf;

use super::metadata::DocumentMetadata;
use crate::file_info::DocumentFormat;
use std::{io::Read, path::Path};
use zip::ZipArchive;

pub(super) fn extract_doc_metadata(path: &Path) -> Option<DocumentMetadata> {
    doc::extract_doc_metadata(path)
}

pub(super) fn build_kindle_preview(
    path: &Path,
    format: DocumentFormat,
) -> Option<crate::preview::PreviewContent> {
    kindle::build_kindle_preview(path, format)
}

pub(super) fn extract_ooxml_metadata<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    format: DocumentFormat,
) -> DocumentMetadata {
    ooxml::extract_ooxml_metadata(archive, format)
}

pub(super) fn extract_open_document_metadata<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    format: DocumentFormat,
) -> DocumentMetadata {
    open_document::extract_open_document_metadata(archive, format)
}

pub(super) fn extract_pages_metadata<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> DocumentMetadata {
    pages::extract_pages_metadata(archive)
}

pub(super) fn extract_pdf_metadata(path: &Path) -> Option<DocumentMetadata> {
    pdf::extract_pdf_metadata(path)
}
