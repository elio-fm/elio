use super::super::{
    common::{first_present_string, local_name, read_zip_entry},
    metadata::DocumentMetadata,
};
use quick_xml::{Reader, events::Event};
use std::{collections::BTreeMap, io::Read};
use zip::ZipArchive;

pub(super) fn extract_pages_metadata<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> DocumentMetadata {
    let mut metadata = DocumentMetadata {
        application: Some("Apple Pages".to_string()),
        variant: detect_pages_variant(archive),
        ..DocumentMetadata::default()
    };

    let properties = [
        "Metadata/Properties.plist",
        "metadata.plist",
        "QuickLook/Metadata.plist",
    ]
    .iter()
    .find_map(|name| read_zip_entry(archive, name).and_then(|xml| parse_plist_dict(&xml)));

    if let Some(fields) = properties {
        metadata.title = first_present_string(
            &fields,
            &["document-title", "kMDItemTitle", "title", "Title"],
            "Title",
        );
        metadata.subject = first_present_string(
            &fields,
            &["subject", "kMDItemDescription", "abstract"],
            "Subject",
        );
        metadata.author = first_present_string(
            &fields,
            &["author", "authors", "kMDItemAuthors", "kMDItemAuthor"],
            "Author",
        );
        metadata.created = first_present_string(
            &fields,
            &["creationDate", "created", "kMDItemContentCreationDate"],
            "Created",
        );
        metadata.modified = first_present_string(
            &fields,
            &[
                "modificationDate",
                "modified",
                "lastOpenedDate",
                "kMDItemContentModificationDate",
            ],
            "Modified",
        );
    }

    metadata
}

fn detect_pages_variant<R: Read + std::io::Seek>(archive: &mut ZipArchive<R>) -> Option<String> {
    let mut saw_iwa = false;
    let mut saw_legacy_index = false;
    for index in 0..archive.len() {
        let Ok(entry) = archive.by_index(index) else {
            continue;
        };
        let name = entry.name().to_ascii_lowercase();
        saw_iwa |= name.ends_with(".iwa");
        saw_legacy_index |= name.ends_with("index.xml") || name.ends_with("index.xml.gz");
    }

    if saw_iwa {
        Some("iWork package".to_string())
    } else if saw_legacy_index {
        Some("Pages '09 package".to_string())
    } else {
        Some("Pages package".to_string())
    }
}

fn parse_plist_dict(xml: &str) -> Option<BTreeMap<String, String>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut fields = BTreeMap::new();
    let mut pending_key: Option<String> = None;
    let mut current_tag: Option<String> = None;
    let mut current_array_key: Option<String> = None;
    let mut array_values = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "array" {
                    current_array_key = pending_key.take();
                    array_values.clear();
                }
                current_tag = Some(tag);
            }
            Ok(Event::Empty(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "true" || tag == "false" {
                    let Some(key) = pending_key.take() else {
                        continue;
                    };
                    fields.insert(key, tag);
                }
                current_tag = None;
            }
            Ok(Event::Text(text)) => {
                let Ok(value) = text.decode() else {
                    continue;
                };
                let value = value.trim();
                if value.is_empty() {
                    continue;
                }
                match current_tag.as_deref() {
                    Some("key") => pending_key = Some(value.to_string()),
                    Some("string") | Some("date") | Some("integer") | Some("real") => {
                        if current_array_key.is_some() {
                            array_values.push(value.to_string());
                        } else if let Some(key) = pending_key.take() {
                            fields.insert(key, value.to_string());
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "array"
                    && let Some(key) = current_array_key.take()
                    && !array_values.is_empty()
                {
                    fields.insert(key, array_values.join(", "));
                    array_values.clear();
                }
                current_tag = None;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    if fields.is_empty() {
        None
    } else {
        Some(fields)
    }
}
