use std::path::Path;

pub(in crate::app::create) struct ParsedCreateItem {
    pub(in crate::app::create) raw: String,
    pub(in crate::app::create) name: String,
    pub(in crate::app::create) is_dir: bool,
}

pub(in crate::app::create) fn parse_create_line(line: &str) -> ParsedCreateItem {
    let is_dir = line.starts_with('/') || line.ends_with('/');
    let name = line.trim_matches('/').to_string();
    ParsedCreateItem {
        raw: line.to_string(),
        name,
        is_dir,
    }
}

pub(in crate::app::create) fn validate_parsed_item(
    item: &ParsedCreateItem,
    cwd: &Path,
) -> Option<String> {
    if item.name.is_empty() {
        return Some("Name cannot be empty".to_string());
    }
    if item.name.contains('/') {
        return Some("Name cannot contain /".to_string());
    }
    if cwd.join(&item.name).exists() {
        return Some(format!("\"{}\" already exists", item.name));
    }
    None
}
