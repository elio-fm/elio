use super::super::{
    common::{
        parse_xml_text_fields, present_count, present_string, push_count_stat, read_zip_entry,
    },
    metadata::DocumentMetadata,
};
use crate::file_info::DocumentFormat;
use std::io::Read;
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
            push_count_stat(&mut metadata, "Slides", present_count(app.get("Slides")));
            push_count_stat(&mut metadata, "Notes", present_count(app.get("Notes")));
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
