use super::super::{
    common::{
        parse_xml_text_fields, present_count, present_string, push_count_stat, read_zip_entry,
    },
    metadata::DocumentMetadata,
};
use crate::file_info::DocumentFormat;
use std::io::{Read, Seek};
use zip::ZipArchive;

pub(super) fn extract_ooxml_metadata<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    format: DocumentFormat,
) -> DocumentMetadata {
    let mut metadata = DocumentMetadata::default();

    let core = read_zip_entry(archive, "docProps/core.xml")
        .map(|xml| parse_xml_text_fields(&xml))
        .unwrap_or_default();
    let app = read_zip_entry(archive, "docProps/app.xml")
        .map(|xml| parse_xml_text_fields(&xml))
        .unwrap_or_default();

    metadata.title = present_string(core.get("title"), "Title");
    metadata.subject = present_string(core.get("subject"), "Subject");
    metadata.author = present_string(core.get("creator"), "Author");
    metadata.modified_by = present_string(core.get("lastModifiedBy"), "Modified By");
    metadata.created = present_string(core.get("created"), "Created");
    metadata.modified = present_string(core.get("modified"), "Modified");
    metadata.application = present_string(app.get("Application"), "Application");
    if let Some(company) = present_string(app.get("Company"), "Company") {
        metadata.metadata.push(("Company".to_string(), company));
    }

    match format {
        DocumentFormat::Docx | DocumentFormat::Docm => {
            push_count_stat(&mut metadata, "Pages", present_count(app.get("Pages")));
            push_count_stat(&mut metadata, "Words", present_count(app.get("Words")));
            push_count_stat(
                &mut metadata,
                "Characters",
                present_count(app.get("Characters")),
            );
        }
        DocumentFormat::Pptx | DocumentFormat::Pptm => {
            let slide_count = archive_part_count(archive, "ppt/slides/slide", ".xml");
            let notes_count = archive_part_count(archive, "ppt/notesSlides/notesSlide", ".xml");
            push_count_stat(
                &mut metadata,
                "Slides",
                count_with_archive_fallback(present_count(app.get("Slides")), slide_count),
            );
            push_count_stat(
                &mut metadata,
                "Notes",
                count_with_archive_fallback(present_count(app.get("Notes")), notes_count),
            );
            push_count_stat(
                &mut metadata,
                "Hidden Slides",
                present_count(app.get("HiddenSlides")),
            );
            push_count_stat(
                &mut metadata,
                "Media Clips",
                present_count(app.get("MMClips")),
            );
        }
        DocumentFormat::Xlsx | DocumentFormat::Xlsm => {
            if let Some(manager) = present_string(app.get("Manager"), "Manager") {
                metadata.metadata.push(("Manager".to_string(), manager));
            }
        }
        _ => {}
    }

    metadata
}

fn archive_part_count<R: Read + Seek>(
    archive: &ZipArchive<R>,
    prefix: &str,
    suffix: &str,
) -> Option<u64> {
    let count = archive
        .file_names()
        .filter(|name| {
            name.strip_prefix(prefix)
                .and_then(|rest| rest.strip_suffix(suffix))
                .is_some_and(|part_number| {
                    !part_number.is_empty() && part_number.bytes().all(|byte| byte.is_ascii_digit())
                })
        })
        .count();
    (count > 0).then_some(count as u64)
}

fn count_with_archive_fallback(app_count: Option<u64>, archive_count: Option<u64>) -> Option<u64> {
    match (app_count, archive_count) {
        (Some(0), Some(count)) if count > 0 => Some(count),
        (Some(count), _) => Some(count),
        (None, count) => count,
    }
}
