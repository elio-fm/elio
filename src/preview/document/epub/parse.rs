use super::super::{
    common::{local_name, present_str, strip_fragment_identifier, xml_attribute_value},
    metadata::DocumentMetadata,
};
use quick_xml::{Reader, events::Event};
use std::collections::BTreeMap;

pub(super) struct EpubPackageDocument {
    pub(super) metadata: DocumentMetadata,
    pub(super) manifest: BTreeMap<String, EpubManifestItem>,
    pub(super) spine: Vec<String>,
    pub(super) nav_path: Option<String>,
    pub(super) ncx_path: Option<String>,
    pub(super) toc_id: Option<String>,
    pub(super) cover_id: Option<String>,
}

impl EpubPackageDocument {
    fn new() -> Self {
        Self {
            metadata: DocumentMetadata {
                variant: Some("EPUB package".to_string()),
                ..DocumentMetadata::default()
            },
            manifest: BTreeMap::new(),
            spine: Vec::new(),
            nav_path: None,
            ncx_path: None,
            toc_id: None,
            cover_id: None,
        }
    }
}

#[derive(Clone)]
pub(super) struct EpubManifestItem {
    pub(super) href: String,
    pub(super) media_type: Option<String>,
    pub(super) properties: Vec<String>,
}

pub(super) fn parse_epub_rootfile_path(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event() {
            Ok(Event::Empty(event)) | Ok(Event::Start(event)) => {
                if local_name(event.name().as_ref()) != "rootfile" {
                    continue;
                }
                for attribute in event.attributes().flatten() {
                    if local_name(attribute.key.as_ref()) != "full-path" {
                        continue;
                    }
                    let value = attribute.decode_and_unescape_value(reader.decoder()).ok()?;
                    let value = value.trim();
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

pub(super) fn parse_epub_package_document(xml: &str) -> EpubPackageDocument {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut package = EpubPackageDocument::new();
    let mut stack = Vec::<String>::new();
    let mut current_metadata_tag: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                match tag.as_str() {
                    "metadata" | "manifest" => {}
                    "spine" => {
                        if package.toc_id.is_none() {
                            package.toc_id = xml_attribute_value(&event, reader.decoder(), "toc");
                        }
                    }
                    "item" if stack.last().is_some_and(|section| section == "manifest") => {
                        register_epub_manifest_item(&mut package, &event, reader.decoder());
                    }
                    "itemref" if stack.last().is_some_and(|section| section == "spine") => {
                        register_epub_spine_itemref(&mut package, &event, reader.decoder());
                    }
                    "meta" if stack.last().is_some_and(|section| section == "metadata") => {
                        register_epub_meta(&mut package, &event, reader.decoder());
                    }
                    "title" | "subject" | "creator" | "language" | "publisher" | "identifier"
                    | "date" => {
                        if stack.last().is_some_and(|section| section == "metadata") {
                            current_metadata_tag = Some(tag.clone());
                        }
                    }
                    _ => {}
                }
                stack.push(tag);
            }
            Ok(Event::Empty(event)) => {
                let tag = local_name(event.name().as_ref());
                match tag.as_str() {
                    "item" if stack.last().is_some_and(|section| section == "manifest") => {
                        register_epub_manifest_item(&mut package, &event, reader.decoder());
                    }
                    "itemref" if stack.last().is_some_and(|section| section == "spine") => {
                        register_epub_spine_itemref(&mut package, &event, reader.decoder());
                    }
                    "meta" if stack.last().is_some_and(|section| section == "metadata") => {
                        register_epub_meta(&mut package, &event, reader.decoder());
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(text)) => {
                let Some(tag) = current_metadata_tag.as_deref() else {
                    continue;
                };
                let Ok(value) = text.decode() else {
                    continue;
                };
                assign_epub_metadata_text(&mut package.metadata, tag, value.as_ref());
            }
            Ok(Event::End(event)) => {
                let tag = local_name(event.name().as_ref());
                if current_metadata_tag.as_deref() == Some(tag.as_str()) {
                    current_metadata_tag = None;
                }
                stack.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    if package.ncx_path.is_none()
        && let Some(toc_id) = package.toc_id.as_ref()
        && let Some(item) = package.manifest.get(toc_id)
    {
        package.ncx_path = Some(item.href.clone());
    }

    package
}

pub(super) fn resolve_epub_cover_item(package: &EpubPackageDocument) -> Option<&EpubManifestItem> {
    package
        .manifest
        .values()
        .find(|item| {
            item.properties
                .iter()
                .any(|property| property == "cover-image")
        })
        .or_else(|| {
            package
                .cover_id
                .as_deref()
                .and_then(|cover_id| package.manifest.get(cover_id))
        })
        .or_else(|| {
            package.manifest.values().find(|item| {
                epub_manifest_item_is_image(item)
                    && strip_fragment_identifier(&item.href)
                        .to_ascii_lowercase()
                        .contains("cover")
            })
        })
}

pub(super) fn epub_manifest_item_is_text(item: &EpubManifestItem) -> bool {
    if item.properties.iter().any(|property| property == "nav") {
        return false;
    }
    match item.media_type.as_deref() {
        Some("application/xhtml+xml")
        | Some("application/xml")
        | Some("text/html")
        | Some("application/x-dtbook+xml") => true,
        _ => {
            let href = strip_fragment_identifier(&item.href).to_ascii_lowercase();
            href.ends_with(".xhtml")
                || href.ends_with(".html")
                || href.ends_with(".htm")
                || href.ends_with(".xml")
        }
    }
}

fn register_epub_manifest_item(
    package: &mut EpubPackageDocument,
    event: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
) {
    let Some(id) = xml_attribute_value(event, decoder, "id") else {
        return;
    };
    let Some(href) = xml_attribute_value(event, decoder, "href") else {
        return;
    };
    let media_type = xml_attribute_value(event, decoder, "media-type");
    let properties = xml_attribute_value(event, decoder, "properties")
        .map(|value| {
            value
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if package.nav_path.is_none() && properties.iter().any(|property| property == "nav") {
        package.nav_path = Some(href.clone());
    }
    if package.ncx_path.is_none() && media_type.as_deref() == Some("application/x-dtbncx+xml") {
        package.ncx_path = Some(href.clone());
    }
    package.manifest.insert(
        id,
        EpubManifestItem {
            href,
            media_type,
            properties,
        },
    );
}

fn register_epub_spine_itemref(
    package: &mut EpubPackageDocument,
    event: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
) {
    if matches!(
        xml_attribute_value(event, decoder, "linear").as_deref(),
        Some("no")
    ) {
        return;
    }
    if let Some(idref) = xml_attribute_value(event, decoder, "idref") {
        package.spine.push(idref);
    }
}

fn register_epub_meta(
    package: &mut EpubPackageDocument,
    event: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
) {
    let name = xml_attribute_value(event, decoder, "name");
    let property = xml_attribute_value(event, decoder, "property");
    let content = xml_attribute_value(event, decoder, "content");

    if name.as_deref() == Some("cover") && package.cover_id.is_none() {
        package.cover_id = content.clone();
    }
    if property.as_deref() == Some("dcterms:modified") && package.metadata.modified.is_none() {
        package.metadata.modified = content.and_then(|value| present_str(&value, "Modified"));
    }
}

fn assign_epub_metadata_text(metadata: &mut DocumentMetadata, tag: &str, value: &str) {
    match tag {
        "title" if metadata.title.is_none() => {
            metadata.title = present_str(value, "Title");
        }
        "subject" if metadata.subject.is_none() => {
            metadata.subject = present_str(value, "Subject");
        }
        "creator" if metadata.author.is_none() => {
            metadata.author = present_str(value, "Author");
        }
        "date" if metadata.created.is_none() => {
            metadata.created = present_str(value, "Created");
        }
        "language" => push_epub_metadata_once(metadata, "Language", value),
        "publisher" => push_epub_metadata_once(metadata, "Publisher", value),
        "identifier" => push_epub_metadata_once(metadata, "Identifier", value),
        _ => {}
    }
}

fn push_epub_metadata_once(metadata: &mut DocumentMetadata, label: &str, value: &str) {
    let Some(value) = present_str(value, label) else {
        return;
    };
    if metadata
        .metadata
        .iter()
        .all(|(existing, _)| existing != label)
    {
        metadata.metadata.push((label.to_string(), value));
    }
}

fn epub_manifest_item_is_image(item: &EpubManifestItem) -> bool {
    matches!(
        item.media_type.as_deref(),
        Some("image/png")
            | Some("image/jpeg")
            | Some("image/gif")
            | Some("image/webp")
            | Some("image/svg+xml")
    ) || matches!(
        strip_fragment_identifier(&item.href).to_ascii_lowercase().as_str(),
        href if href.ends_with(".png")
            || href.ends_with(".jpg")
            || href.ends_with(".jpeg")
            || href.ends_with(".gif")
            || href.ends_with(".webp")
            || href.ends_with(".svg")
    )
}
