use super::super::common::{
    local_name, read_zip_entry_limited, resolve_zip_entry_path, strip_fragment_identifier,
    xml_attribute_value,
};
use super::{
    EPUB_NAV_ENTRY_LIMIT_BYTES, append_epub_text_fragment,
    parse::{EpubPackageDocument, epub_manifest_item_is_text},
};
use quick_xml::{Reader, events::Event};
use std::{io::Read, path::Path};
use zip::ZipArchive;

#[derive(Clone, Debug)]
pub(super) struct EpubSection {
    pub(super) path: String,
    pub(super) title: Option<String>,
}

#[derive(Clone)]
struct EpubNavPoint {
    href: Option<String>,
    label: String,
}

pub(super) fn build_epub_sections<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    package: &EpubPackageDocument,
    package_path: &str,
) -> Vec<EpubSection> {
    let nav_points = extract_epub_table_of_contents(archive, package, package_path);
    let titles_by_path = nav_points
        .iter()
        .filter_map(|point| {
            point.href.as_deref().map(|href| {
                (
                    resolve_zip_entry_path(package_path, href),
                    point.label.trim().to_string(),
                )
            })
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    let fallback_titles = nav_points
        .iter()
        .map(|point| point.label.trim())
        .filter(|label| !label.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut fallback_index = 0usize;
    let mut sections = Vec::new();

    for idref in &package.spine {
        let Some(item) = package.manifest.get(idref) else {
            continue;
        };
        if !epub_manifest_item_is_text(item) {
            continue;
        }

        let path = resolve_zip_entry_path(package_path, &item.href);
        let title = titles_by_path.get(&path).cloned().or_else(|| {
            let title = fallback_titles.get(fallback_index).cloned();
            fallback_index += 1;
            title
        });
        sections.push(EpubSection { path, title });
    }

    sections
}

pub(super) fn epub_section_title_from_path(path: &str) -> Option<String> {
    Path::new(strip_fragment_identifier(path))
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.replace(['_', '-'], " "))
        .map(|stem| stem.trim().to_string())
        .filter(|stem| !stem.is_empty())
}

fn extract_epub_table_of_contents<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    package: &EpubPackageDocument,
    package_path: &str,
) -> Vec<EpubNavPoint> {
    let nav_href = package.nav_path.as_deref().or(package.ncx_path.as_deref());
    let Some(nav_href) = nav_href else {
        return Vec::new();
    };
    let resolved = resolve_zip_entry_path(package_path, nav_href);
    let Some(xml) = read_zip_entry_limited(archive, &resolved, EPUB_NAV_ENTRY_LIMIT_BYTES) else {
        return Vec::new();
    };

    if package.nav_path.as_deref() == Some(nav_href) {
        parse_epub_nav_toc(&xml)
    } else {
        parse_ncx_toc(&xml)
    }
}

fn parse_epub_nav_toc(xml: &str) -> Vec<EpubNavPoint> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut nav_stack = Vec::<bool>::new();
    let mut item_depth = 0usize;
    let mut current_label = String::new();
    let mut current_href: Option<String> = None;
    let mut items = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "nav" {
                    nav_stack.push(epub_nav_is_toc(&event, reader.decoder()));
                    continue;
                }
                if !epub_nav_stack_active(&nav_stack) {
                    continue;
                }
                if tag == "li" {
                    item_depth += 1;
                    if item_depth == 1 {
                        current_label.clear();
                        current_href = None;
                    }
                } else if tag == "a" && item_depth > 0 && current_href.is_none() {
                    current_href = xml_attribute_value(&event, reader.decoder(), "href");
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = local_name(event.name().as_ref());
                if !epub_nav_stack_active(&nav_stack) || item_depth == 0 {
                    continue;
                }
                if tag == "br" {
                    append_epub_text_fragment(&mut current_label, " ");
                } else if tag == "a" && current_href.is_none() {
                    current_href = xml_attribute_value(&event, reader.decoder(), "href");
                }
            }
            Ok(Event::Text(text)) => {
                if !epub_nav_stack_active(&nav_stack) || item_depth == 0 {
                    continue;
                }
                if let Ok(value) = text.decode() {
                    append_epub_text_fragment(&mut current_label, value.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "li" && epub_nav_stack_active(&nav_stack) {
                    if item_depth == 1 {
                        push_epub_nav_item(&mut items, current_href.take(), &current_label);
                    }
                    item_depth = item_depth.saturating_sub(1);
                    continue;
                }
                if tag == "nav" {
                    let completed = nav_stack.pop().unwrap_or(false);
                    if completed && !items.is_empty() {
                        break;
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    items
}

fn parse_ncx_toc(xml: &str) -> Vec<EpubNavPoint> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut nav_depth = 0usize;
    let mut in_label = false;
    let mut in_text = false;
    let mut current_label = String::new();
    let mut current_href: Option<String> = None;
    let mut items = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                match tag.as_str() {
                    "navPoint" => {
                        nav_depth += 1;
                        if nav_depth == 1 {
                            current_label.clear();
                            current_href = None;
                        }
                    }
                    "navLabel" if nav_depth > 0 => {
                        in_label = true;
                        current_label.clear();
                    }
                    "text" if in_label => in_text = true,
                    "content" if nav_depth > 0 && current_href.is_none() => {
                        current_href = xml_attribute_value(&event, reader.decoder(), "src");
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(event)) => {
                if nav_depth > 0 && local_name(event.name().as_ref()) == "content" {
                    current_href = xml_attribute_value(&event, reader.decoder(), "src");
                }
            }
            Ok(Event::Text(text)) => {
                if in_label
                    && in_text
                    && let Ok(value) = text.decode()
                {
                    append_epub_text_fragment(&mut current_label, value.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let tag = local_name(event.name().as_ref());
                match tag.as_str() {
                    "text" => in_text = false,
                    "navLabel" => in_label = false,
                    "navPoint" => {
                        if nav_depth == 1 {
                            push_epub_nav_item(&mut items, current_href.take(), &current_label);
                        }
                        nav_depth = nav_depth.saturating_sub(1);
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    items
}

fn epub_nav_is_toc(
    event: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
) -> bool {
    event.attributes().flatten().any(|attribute| {
        let key = local_name(attribute.key.as_ref());
        let Ok(value) = attribute.decode_and_unescape_value(decoder) else {
            return false;
        };
        let value = value.trim();
        (key == "type" && value.split_whitespace().any(|token| token == "toc"))
            || (key == "role" && value.split_whitespace().any(|token| token == "doc-toc"))
    })
}

fn epub_nav_stack_active(nav_stack: &[bool]) -> bool {
    nav_stack.last().copied().unwrap_or(false)
}

fn push_epub_nav_item(items: &mut Vec<EpubNavPoint>, href: Option<String>, label: &str) {
    let label = label.trim();
    if label.is_empty() {
        return;
    }
    if items
        .last()
        .is_some_and(|existing| existing.label == label && existing.href == href)
    {
        return;
    }
    items.push(EpubNavPoint {
        href,
        label: label.to_string(),
    });
}
