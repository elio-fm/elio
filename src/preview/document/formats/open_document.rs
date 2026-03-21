use super::super::{
    common::{
        parse_xml_text_fields, present_count, present_string, push_count_stat, read_zip_entry,
    },
    metadata::DocumentMetadata,
};
use crate::file_info::DocumentFormat;
use std::io::Read;
use zip::ZipArchive;

pub(super) fn extract_open_document_metadata<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    format: DocumentFormat,
) -> DocumentMetadata {
    let mut metadata = DocumentMetadata::default();
    let Some(xml) = read_zip_entry(archive, "meta.xml") else {
        return metadata;
    };

    let fields = parse_xml_text_fields(&xml);
    metadata.title = present_string(fields.get("title"), "Title");
    metadata.subject = present_string(fields.get("subject"), "Subject");
    metadata.author = present_string(
        fields.get("initial-creator").or(fields.get("creator")),
        "Author",
    );
    metadata.created = present_string(fields.get("creation-date"), "Created");
    metadata.modified = present_string(fields.get("date"), "Modified");
    metadata.application = present_string(fields.get("generator"), "Application");

    match format {
        DocumentFormat::Odt => {
            push_count_stat(
                &mut metadata,
                "Pages",
                present_count(fields.get("page-count")),
            );
            push_count_stat(
                &mut metadata,
                "Words",
                present_count(fields.get("word-count")),
            );
            push_count_stat(
                &mut metadata,
                "Characters",
                present_count(fields.get("character-count")),
            );
        }
        DocumentFormat::Ods => {
            push_count_stat(
                &mut metadata,
                "Tables",
                present_count(fields.get("table-count")),
            );
            push_count_stat(
                &mut metadata,
                "Cells",
                present_count(fields.get("cell-count")),
            );
            push_count_stat(
                &mut metadata,
                "Objects",
                present_count(fields.get("object-count")),
            );
        }
        DocumentFormat::Odp => {
            push_count_stat(
                &mut metadata,
                "Slides",
                present_count(fields.get("page-count")),
            );
            push_count_stat(
                &mut metadata,
                "Objects",
                present_count(fields.get("object-count")),
            );
        }
        _ => {}
    }

    metadata
}
