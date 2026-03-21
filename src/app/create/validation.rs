use std::path::Path;

pub(super) struct ParsedCreateItem {
    pub(super) raw: String,
    pub(super) name: String,
    pub(super) is_dir: bool,
}

pub(super) fn parse_create_line(line: &str) -> ParsedCreateItem {
    let is_dir = line.starts_with('/') || line.ends_with('/');
    let name = line.trim_matches('/').to_string();
    ParsedCreateItem {
        raw: line.to_string(),
        name,
        is_dir,
    }
}

pub(super) fn validate_parsed_item(item: &ParsedCreateItem, cwd: &Path) -> Option<String> {
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
