use super::{
    epub::build_epub_preview,
    kindle::build_kindle_preview,
    metadata_preview::render_document_preview,
    office::{
        extract_apple_pages_metadata, extract_legacy_word_metadata,
        extract_microsoft_office_metadata, extract_open_document_metadata,
    },
    pdf::extract_pdf_metadata,
    zip_reading::extract_zip_document_metadata,
};
use crate::{file_classification::DocumentFormat, preview::PreviewContent};
use std::path::Path;

pub(in crate::preview) fn build_document_preview(
    path: &Path,
    format: DocumentFormat,
    epub_section_index: Option<usize>,
) -> Option<PreviewContent> {
    let metadata = match format {
        DocumentFormat::Doc => extract_legacy_word_metadata(path),
        DocumentFormat::Docx | DocumentFormat::Docm => {
            extract_zip_document_metadata(path, |archive| {
                extract_microsoft_office_metadata(archive, format)
            })
        }
        DocumentFormat::Odt | DocumentFormat::Ods | DocumentFormat::Odp => {
            extract_zip_document_metadata(path, |archive| {
                extract_open_document_metadata(archive, format)
            })
        }
        DocumentFormat::Pptx | DocumentFormat::Pptm => {
            extract_zip_document_metadata(path, |archive| {
                extract_microsoft_office_metadata(archive, format)
            })
        }
        DocumentFormat::Xlsx | DocumentFormat::Xlsm => {
            extract_zip_document_metadata(path, |archive| {
                extract_microsoft_office_metadata(archive, format)
            })
        }
        DocumentFormat::Pages => extract_zip_document_metadata(path, extract_apple_pages_metadata),
        DocumentFormat::Epub => {
            return build_epub_preview(path, epub_section_index.unwrap_or(0));
        }
        DocumentFormat::Mobi | DocumentFormat::Azw3 => return build_kindle_preview(path, format),
        DocumentFormat::Pdf => extract_pdf_metadata(path),
    }?;

    Some(render_document_preview(format, metadata))
}

#[cfg(test)]
pub(in crate::preview) fn reset_epub_package_parse_count(path: &Path) {
    super::epub::reset_epub_package_parse_count(path);
}

#[cfg(test)]
pub(in crate::preview) fn epub_package_parse_count(path: &Path) -> usize {
    super::epub::epub_package_parse_count(path)
}

#[cfg(test)]
pub(in crate::preview) fn clear_epub_package_cache() {
    super::epub::clear_epub_package_cache();
}
