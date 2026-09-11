use super::archive_reading::ComicArchivePage;
use std::{collections::BTreeSet, path::Path};

#[derive(Clone, Debug, Default)]
pub(super) struct ComicDerivedMetadata {
    pub(super) series: Option<String>,
    pub(super) volume: Option<String>,
    pub(super) number: Option<String>,
    pub(super) year: Option<String>,
    pub(super) publisher: Option<String>,
    pub(super) source: Option<String>,
    pub(super) chapters: Option<String>,
}

impl ComicDerivedMetadata {
    fn has_visible_fields(&self) -> bool {
        self.series.is_some()
            && (self.volume.is_some()
                || self.number.is_some()
                || self.year.is_some()
                || self.publisher.is_some()
                || self.source.is_some()
                || self.chapters.is_some())
    }
}

pub(super) fn derive_comic_archive_metadata(
    path: &Path,
    page_entries: &[ComicArchivePage],
) -> Option<ComicDerivedMetadata> {
    let mut metadata = ComicDerivedMetadata::default();
    if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
        apply_archive_name_metadata(&mut metadata, stem);
    }
    if metadata.series.is_none()
        && (metadata.volume.is_some() || metadata.number.is_some())
        && let Some(parent_series) = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .and_then(series_from_collection_folder)
    {
        metadata.series = Some(parent_series);
    }
    apply_page_entry_metadata(&mut metadata, page_entries);
    metadata.has_visible_fields().then_some(metadata)
}

fn apply_archive_name_metadata(metadata: &mut ComicDerivedMetadata, stem: &str) {
    let annotations = bracketed_tokens(stem, '(', ')')
        .into_iter()
        .chain(bracketed_tokens(stem, '[', ']'));
    for token in annotations {
        if metadata.year.is_none() && is_year_token(&token) {
            metadata.year = Some(token);
        } else if metadata.source.is_none() && is_source_token(&token) {
            metadata.source = Some(normalize_source_token(&token));
        } else if metadata.publisher.is_none() && looks_like_publisher_tag(&token) {
            metadata.publisher = Some(token.trim().to_string());
        }
    }

    let main = clean_archive_name_main(stem);
    if main.is_empty() {
        return;
    }

    if let Some((series, volume)) = split_series_and_prefixed_number(main, 'v')
        .or_else(|| split_series_and_prefixed_number(main, 't'))
    {
        set_derived_series(metadata, series);
        metadata.volume.get_or_insert(volume);
    } else if let Some((series, volume)) = split_series_and_volume_label(main) {
        if let Some(series) = series {
            set_derived_series(metadata, series);
        }
        metadata.volume.get_or_insert(volume);
    } else if let Some((series, number)) = split_series_and_issue_number(main) {
        set_derived_series(metadata, series);
        metadata.number.get_or_insert(number);
    } else if (metadata.year.is_some() || metadata.source.is_some() || metadata.publisher.is_some())
        && is_meaningful_series_candidate(main)
    {
        metadata.series.get_or_insert_with(|| main.to_string());
    }
}

fn clean_archive_name_main(stem: &str) -> &str {
    let mut main = stem.trim();
    while let Some(stripped) = strip_leading_bracketed_token(main) {
        main = stripped.trim_start();
    }
    main.split(" (")
        .next()
        .unwrap_or(main)
        .split(" [")
        .next()
        .unwrap_or(main)
        .trim()
}

fn set_derived_series(metadata: &mut ComicDerivedMetadata, series: String) {
    if is_meaningful_series_candidate(&series) {
        metadata.series.get_or_insert(series);
    }
}

fn apply_page_entry_metadata(
    metadata: &mut ComicDerivedMetadata,
    page_entries: &[ComicArchivePage],
) {
    let mut chapters = BTreeSet::new();
    let mut page_series: Option<String> = None;

    for page in page_entries {
        let stem = archive_entry_stem(&page.entry_name);
        if let Some(chapter) = extract_prefixed_number(stem, 'c') {
            chapters.insert(chapter);
        }
        if metadata.volume.is_none()
            && let Some(volume) = extract_prefixed_number(stem, 'v')
        {
            metadata.volume = Some(format_number_without_padding(volume));
        }
        if page_series.is_none()
            && let Some(series) = series_from_page_entry_stem(stem)
        {
            page_series = Some(series);
        }

        let tags = bracketed_tokens(stem, '[', ']');
        if metadata.source.is_none()
            && let Some(source) = tags.iter().find(|tag| is_source_token(tag))
        {
            metadata.source = Some(normalize_source_token(source));
        }
        if metadata.publisher.is_none()
            && let Some(publisher) = tags.iter().find(|tag| looks_like_publisher_tag(tag))
        {
            metadata.publisher = Some(publisher.trim().to_string());
        }
    }

    if metadata.series.is_none() {
        metadata.series = page_series;
    }
    metadata.chapters = summarize_number_set(&chapters);
}

fn split_series_and_prefixed_number(value: &str, prefix: char) -> Option<(String, String)> {
    let (series, suffix) = value.rsplit_once(' ')?;
    let mut chars = suffix.chars();
    let first = chars.next()?;
    if !first.eq_ignore_ascii_case(&prefix) {
        return None;
    }
    let number = chars.as_str();
    if number.is_empty() || !number.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let series = series.trim();
    (!series.is_empty()).then(|| (series.to_string(), strip_numeric_padding(number)))
}

fn split_series_and_volume_label(value: &str) -> Option<(Option<String>, String)> {
    let (prefix, number) = value.rsplit_once(' ')?;
    if number.is_empty() || !number.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }

    let (series, label) = prefix
        .rsplit_once(' ')
        .map(|(series, label)| (series.trim(), label.trim()))
        .unwrap_or(("", prefix.trim()));
    if !matches!(
        label.to_ascii_lowercase().as_str(),
        "volume" | "vol" | "vol."
    ) {
        return None;
    }

    let series = (!series.is_empty()).then(|| series.to_string());
    Some((series, strip_numeric_padding(number)))
}

fn split_series_and_issue_number(value: &str) -> Option<(String, String)> {
    let (series, suffix) = value.rsplit_once(" #")?;
    if suffix.is_empty() || !suffix.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let series = series.trim();
    (!series.is_empty()).then(|| (series.to_string(), strip_numeric_padding(suffix)))
}

fn series_from_collection_folder(value: &str) -> Option<String> {
    let mut name = value.trim();
    while let Some(stripped) = strip_leading_bracketed_token(name) {
        name = stripped.trim_start();
    }
    let name = name
        .split(" (")
        .next()
        .unwrap_or(name)
        .split(" [")
        .next()
        .unwrap_or(name)
        .trim();
    is_meaningful_series_candidate(name).then(|| name.to_string())
}

fn strip_leading_bracketed_token(value: &str) -> Option<&str> {
    let value = value.strip_prefix('[')?;
    let (_, rest) = value.split_once(']')?;
    Some(rest)
}

fn is_meaningful_series_candidate(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    if value.len() >= 12 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return false;
    }
    !matches!(
        value.to_ascii_lowercase().as_str(),
        "archive"
            | "archives"
            | "book"
            | "books"
            | "cbz"
            | "chapter"
            | "comic"
            | "comics"
            | "digital"
            | "download"
            | "downloads"
            | "issue"
            | "manga"
            | "pages"
            | "scan"
            | "scans"
            | "volume"
    )
}

fn archive_entry_stem(entry_name: &str) -> &str {
    let name = entry_name.rsplit(['/', '\\']).next().unwrap_or(entry_name);
    name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name)
}

fn series_from_page_entry_stem(stem: &str) -> Option<String> {
    let lower = stem.to_lowercase();
    let bytes = lower.as_bytes();
    for index in 0..bytes.len().saturating_sub(4) {
        if bytes.get(index..index + 4) == Some(b" - c")
            && bytes.get(index + 4).is_some_and(u8::is_ascii_digit)
        {
            let series = stem[..index].trim();
            return (!series.is_empty()).then(|| series.to_string());
        }
    }
    None
}

fn extract_prefixed_number(value: &str, prefix: char) -> Option<u32> {
    let bytes = value.as_bytes();
    let prefix = prefix.to_ascii_lowercase() as u8;
    for index in 0..bytes.len().saturating_sub(1) {
        if bytes[index].to_ascii_lowercase() != prefix
            || !bytes[index + 1].is_ascii_digit()
            || (index > 0 && bytes[index - 1].is_ascii_alphanumeric())
        {
            continue;
        }

        let start = index + 1;
        let mut end = start;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if let Ok(number) = value[start..end].parse::<u32>() {
            return Some(number);
        }
    }
    None
}

fn bracketed_tokens(value: &str, open: char, close: char) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    for ch in value.chars() {
        if ch == open {
            current.clear();
            in_token = true;
        } else if ch == close && in_token {
            let token = current.trim();
            if !token.is_empty() {
                tokens.push(token.to_string());
            }
            current.clear();
            in_token = false;
        } else if in_token {
            current.push(ch);
        }
    }
    tokens
}

fn summarize_number_set(numbers: &BTreeSet<u32>) -> Option<String> {
    let first = numbers.first()?;
    let last = numbers.last()?;
    Some(if first == last {
        format_number_without_padding(*first)
    } else {
        format!(
            "{}-{}",
            format_number_without_padding(*first),
            format_number_without_padding(*last)
        )
    })
}

pub(super) fn is_year_token(value: &str) -> bool {
    value.len() == 4
        && value.chars().all(|ch| ch.is_ascii_digit())
        && value
            .parse::<u16>()
            .is_ok_and(|year| (1900..=2100).contains(&year))
}

fn is_source_token(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "digital" | "digital edition" | "web" | "web-dl" | "print"
    ) || lower.starts_with("digital-")
        || lower.starts_with("digital ")
}

fn normalize_source_token(value: &str) -> String {
    let lower = value.trim().to_ascii_lowercase();
    if lower.starts_with("digital-") || lower.starts_with("digital ") {
        return "Digital".to_string();
    }
    match lower.as_str() {
        "digital" | "digital edition" => "Digital".to_string(),
        "web" | "web-dl" => "Web".to_string(),
        "print" => "Print".to_string(),
        _ => value.trim().to_string(),
    }
}

fn looks_like_publisher_tag(value: &str) -> bool {
    let words: Vec<String> = value
        .trim()
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| word.to_ascii_lowercase())
        .collect();
    let Some(last) = words.last().map(String::as_str) else {
        return false;
    };
    matches!(
        last,
        "press"
            | "publisher"
            | "publishers"
            | "publishing"
            | "comic"
            | "comics"
            | "studio"
            | "studios"
            | "books"
    )
}

fn strip_numeric_padding(value: &str) -> String {
    let stripped = value.trim_start_matches('0');
    if stripped.is_empty() {
        "0".to_string()
    } else {
        stripped.to_string()
    }
}

fn format_number_without_padding(value: u32) -> String {
    value.to_string()
}
