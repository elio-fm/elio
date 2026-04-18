mod common;
mod epub;
mod formats;
mod metadata;

use self::{
    common::extract_zip_document_metadata,
    formats::{
        build_kindle_preview, extract_doc_metadata, extract_ooxml_metadata,
        extract_open_document_metadata, extract_pages_metadata, extract_pdf_metadata,
    },
    metadata::render_document_preview,
};
use super::PreviewContent;
use crate::file_info::DocumentFormat;
use std::path::Path;

pub(super) fn build_document_preview(
    path: &Path,
    format: DocumentFormat,
    epub_section_index: Option<usize>,
) -> Option<PreviewContent> {
    let metadata = match format {
        DocumentFormat::Doc => extract_doc_metadata(path),
        DocumentFormat::Docx | DocumentFormat::Docm => {
            extract_zip_document_metadata(path, |archive| extract_ooxml_metadata(archive, format))
        }
        DocumentFormat::Odt | DocumentFormat::Ods | DocumentFormat::Odp => {
            extract_zip_document_metadata(path, |archive| {
                extract_open_document_metadata(archive, format)
            })
        }
        DocumentFormat::Pptx | DocumentFormat::Pptm => {
            extract_zip_document_metadata(path, |archive| extract_ooxml_metadata(archive, format))
        }
        DocumentFormat::Xlsx | DocumentFormat::Xlsm => {
            extract_zip_document_metadata(path, |archive| extract_ooxml_metadata(archive, format))
        }
        DocumentFormat::Pages => extract_zip_document_metadata(path, extract_pages_metadata),
        DocumentFormat::Epub => {
            return epub::build_epub_preview(path, epub_section_index.unwrap_or(0));
        }
        DocumentFormat::Mobi | DocumentFormat::Azw3 => return build_kindle_preview(path, format),
        DocumentFormat::Pdf => extract_pdf_metadata(path),
    }?;

    Some(render_document_preview(format, metadata))
}

#[cfg(test)]
pub(super) fn reset_epub_package_parse_count(path: &Path) {
    epub::reset_epub_package_parse_count(path);
}

#[cfg(test)]
pub(super) fn epub_package_parse_count(path: &Path) -> usize {
    epub::epub_package_parse_count(path)
}

#[cfg(test)]
pub(super) fn clear_epub_package_cache() {
    epub::clear_epub_package_cache();
}
