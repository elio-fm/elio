use super::types::{
    ComicArchivePage, ComicDerivedMetadata, ComicInfoMetadata, ComicMetadataEntry,
    ComicMetadataFileKind,
};
use quick_xml::{Reader, events::Event};
use serde_json::Value as JsonValue;
use std::{collections::BTreeSet, path::Path};

pub(super) fn capture_comic_metadata_entry(
    metadata_entry: &mut Option<ComicMetadataEntry>,
    entry_name: &str,
) {
    let Some(kind) = comic_metadata_file_kind(entry_name) else {
        return;
    };
    if metadata_entry
        .as_ref()
        .is_some_and(|entry| entry.kind.priority() <= kind.priority())
    {
        return;
    }
    *metadata_entry = Some(ComicMetadataEntry {
        name: entry_name.to_string(),
        kind,
    });
}

fn comic_metadata_file_kind(entry_name: &str) -> Option<ComicMetadataFileKind> {
    let name = entry_name
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .map(|name| name.to_ascii_lowercase())?;
    match name.as_str() {
        "comicinfo.xml" => Some(ComicMetadataFileKind::ComicInfo),
        "metroninfo.xml" => Some(ComicMetadataFileKind::MetronInfo),
        "comet.xml" | "cometinfo.xml" => Some(ComicMetadataFileKind::CoMet),
        _ if name.ends_with(".acbf") => Some(ComicMetadataFileKind::Acbf),
        _ => None,
    }
}

impl ComicMetadataFileKind {
    pub(super) fn priority(self) -> u8 {
        match self {
            Self::ComicInfo => 0,
            Self::MetronInfo => 1,
            Self::CoMet => 2,
            Self::Acbf => 3,
        }
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

fn is_year_token(value: &str) -> bool {
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

#[derive(Debug, Default)]
struct ComicXmlParseState {
    metadata: ComicInfoMetadata,
    path: Vec<String>,
    current_credit: Option<ComicCreditDraft>,
    current_acbf_author: Option<AcbfAuthorDraft>,
}

#[derive(Debug, Default)]
struct ComicCreditDraft {
    creator: Option<String>,
    roles: Vec<String>,
}

#[derive(Debug, Default)]
struct AcbfAuthorDraft {
    activity: Option<String>,
    first_name: Option<String>,
    middle_name: Option<String>,
    last_name: Option<String>,
    nickname: Option<String>,
}

pub(super) fn parse_comic_metadata_xml(xml: &str) -> Option<ComicInfoMetadata> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut state = ComicXmlParseState::default();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = xml_local_name(event.name().as_ref()).to_ascii_lowercase();
                if tag == "credit" {
                    state.current_credit = Some(ComicCreditDraft::default());
                }
                if tag == "author"
                    && state
                        .path
                        .last()
                        .is_some_and(|parent| parent == "book-info")
                {
                    state.current_acbf_author = Some(AcbfAuthorDraft {
                        activity: xml_attribute_value(&event, reader.decoder(), "activity"),
                        ..Default::default()
                    });
                }
                if tag == "sequence"
                    && state
                        .path
                        .last()
                        .is_some_and(|parent| parent == "book-info")
                {
                    set_comic_info_field(
                        &mut state.metadata.series,
                        xml_attribute_value(&event, reader.decoder(), "title").as_deref(),
                    );
                    set_comic_info_field(
                        &mut state.metadata.volume,
                        xml_attribute_value(&event, reader.decoder(), "volume").as_deref(),
                    );
                }
                if tag == "publish-date"
                    && state
                        .path
                        .last()
                        .is_some_and(|parent| parent == "publish-info")
                    && let Some(value) = xml_attribute_value(&event, reader.decoder(), "value")
                {
                    set_comic_info_year_from_date(&mut state.metadata.year, &value);
                }
                state.path.push(tag);
            }
            Ok(Event::Text(text)) => {
                if let Ok(value) = text.decode() {
                    assign_comic_xml_text(&mut state, value.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if let Ok(value) = text.decode() {
                    assign_comic_xml_text(&mut state, value.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let tag = xml_local_name(event.name().as_ref()).to_ascii_lowercase();
                if tag == "credit" {
                    apply_comic_xml_credit(&mut state);
                }
                if tag == "author" {
                    apply_acbf_author(&mut state);
                }
                state.path.pop();
            }
            Ok(Event::Empty(_)) => {}
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    state
        .metadata
        .has_visible_fields()
        .then_some(state.metadata)
}

pub(super) fn parse_comic_book_info_comment(comment: &[u8]) -> Option<ComicInfoMetadata> {
    let text = std::str::from_utf8(comment).ok()?.trim();
    if text.is_empty() {
        return None;
    }
    let root = parse_comic_book_info_json(text)?;
    let info = root.get("ComicBookInfo/1.0")?;
    let mut metadata = ComicInfoMetadata::default();
    set_comic_info_field(
        &mut metadata.title,
        json_metadata_value(info.get("title")).as_deref(),
    );
    set_comic_info_field(
        &mut metadata.series,
        json_metadata_value(info.get("series")).as_deref(),
    );
    set_comic_info_field(
        &mut metadata.number,
        json_metadata_value(info.get("issue")).as_deref(),
    );
    set_comic_info_field(
        &mut metadata.volume,
        json_metadata_value(info.get("volume")).as_deref(),
    );
    set_comic_info_field(
        &mut metadata.year,
        json_metadata_value(info.get("publicationYear")).as_deref(),
    );
    set_comic_info_field(
        &mut metadata.publisher,
        json_metadata_value(info.get("publisher")).as_deref(),
    );
    set_comic_info_field(
        &mut metadata.genre,
        json_metadata_value(info.get("genre")).as_deref(),
    );

    if let Some(credits) = info.get("credits").and_then(JsonValue::as_array) {
        for credit in credits {
            let role = json_metadata_value(credit.get("role")).unwrap_or_default();
            let person = json_metadata_value(credit.get("person")).unwrap_or_default();
            match role.to_ascii_lowercase().as_str() {
                "writer" | "author" => set_comic_info_field(&mut metadata.writer, Some(&person)),
                "penciller" | "artist" => {
                    set_comic_info_field(&mut metadata.penciller, Some(&person));
                }
                _ => {}
            }
        }
    }

    metadata.has_visible_fields().then_some(metadata)
}

fn parse_comic_book_info_json(text: &str) -> Option<JsonValue> {
    if let Ok(root) = serde_json::from_str::<JsonValue>(text)
        && root.get("ComicBookInfo/1.0").is_some()
    {
        return Some(root);
    }

    let marker_index = text.find("\"ComicBookInfo/1.0\"")?;
    let mut starts = text[..marker_index]
        .match_indices('{')
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    starts.reverse();
    starts.into_iter().find_map(|start| {
        balanced_json_object_from(text, start)
            .and_then(|json| serde_json::from_str::<JsonValue>(json).ok())
            .filter(|root| root.get("ComicBookInfo/1.0").is_some())
    })
}

fn balanced_json_object_from(text: &str, start: usize) -> Option<&str> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, ch) in text[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    let end = start + offset + ch.len_utf8();
                    return Some(&text[start..end]);
                }
            }
            _ => {}
        }
    }
    None
}

fn json_metadata_value(value: Option<&JsonValue>) -> Option<String> {
    match value? {
        JsonValue::String(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        }
        JsonValue::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn assign_comic_xml_text(state: &mut ComicXmlParseState, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    let Some(tag) = state.path.last().cloned() else {
        return;
    };

    if capture_comic_xml_credit_text(state, &tag, value) {
        return;
    }
    if capture_acbf_author_text(state, &tag, value) {
        return;
    }

    let parent = state
        .path
        .len()
        .checked_sub(2)
        .and_then(|index| state.path.get(index))
        .map(String::as_str);
    if state.path.len() <= 2 {
        assign_flat_comic_xml_text(&mut state.metadata, &tag, value);
        return;
    }

    match (parent, tag.as_str()) {
        (Some("series"), "name") => set_comic_info_field(&mut state.metadata.series, Some(value)),
        (Some("publisher"), "name") => {
            set_comic_info_field(&mut state.metadata.publisher, Some(value));
        }
        (Some("genres"), "genre") => set_comic_info_field(&mut state.metadata.genre, Some(value)),
        (Some("stories"), "story") => set_comic_info_field(&mut state.metadata.title, Some(value)),
        (Some("book-info"), "book-title") => {
            set_comic_info_field(&mut state.metadata.title, Some(value));
        }
        (Some("book-info"), "genre") => {
            set_comic_info_field(&mut state.metadata.genre, Some(value));
        }
        (Some("book-info"), "sequence") => {
            set_comic_info_field(&mut state.metadata.number, Some(value));
        }
        (Some("publish-info"), "publisher") => {
            set_comic_info_field(&mut state.metadata.publisher, Some(value));
        }
        (Some("publish-info"), "publish-date") => {
            set_comic_info_year_from_date(&mut state.metadata.year, value);
        }
        (Some("series"), "volume") | (_, "mangavolume") => {
            set_comic_info_field(&mut state.metadata.volume, Some(value));
        }
        (_, "coverdate") | (_, "storedate") => {
            set_comic_info_year_from_date(&mut state.metadata.year, value);
        }
        _ => {}
    }
}

fn assign_flat_comic_xml_text(metadata: &mut ComicInfoMetadata, tag: &str, value: &str) {
    match tag {
        "title" | "collectiontitle" => set_comic_info_field(&mut metadata.title, Some(value)),
        "series" => set_comic_info_field(&mut metadata.series, Some(value)),
        "number" | "issue" => set_comic_info_field(&mut metadata.number, Some(value)),
        "volume" | "mangavolume" => set_comic_info_field(&mut metadata.volume, Some(value)),
        "year" => set_comic_info_field(&mut metadata.year, Some(value)),
        "date" | "coverdate" | "storedate" => {
            set_comic_info_year_from_date(&mut metadata.year, value)
        }
        "publisher" => set_comic_info_field(&mut metadata.publisher, Some(value)),
        "writer" | "author" => set_comic_info_field(&mut metadata.writer, Some(value)),
        "penciller" | "artist" => set_comic_info_field(&mut metadata.penciller, Some(value)),
        "genre" => set_comic_info_field(&mut metadata.genre, Some(value)),
        _ => {}
    }
}

fn capture_acbf_author_text(state: &mut ComicXmlParseState, tag: &str, value: &str) -> bool {
    if state.current_acbf_author.is_none() {
        return false;
    }
    let parent = state
        .path
        .len()
        .checked_sub(2)
        .and_then(|index| state.path.get(index))
        .map(String::as_str);
    if parent != Some("author") {
        return false;
    }
    let Some(author) = state.current_acbf_author.as_mut() else {
        return false;
    };
    match tag {
        "first-name" => set_comic_info_field(&mut author.first_name, Some(value)),
        "middle-name" => set_comic_info_field(&mut author.middle_name, Some(value)),
        "last-name" => set_comic_info_field(&mut author.last_name, Some(value)),
        "nickname" => set_comic_info_field(&mut author.nickname, Some(value)),
        _ => return false,
    }
    true
}

fn capture_comic_xml_credit_text(state: &mut ComicXmlParseState, tag: &str, value: &str) -> bool {
    if state.current_credit.is_none() {
        return false;
    }
    let parent = state
        .path
        .len()
        .checked_sub(2)
        .and_then(|index| state.path.get(index))
        .map(String::as_str);
    match (parent, tag) {
        (Some("credit"), "creator") => {
            if let Some(credit) = state.current_credit.as_mut() {
                set_comic_info_field(&mut credit.creator, Some(value));
            }
            true
        }
        (Some("roles"), "role") => {
            if let Some(credit) = state.current_credit.as_mut() {
                credit.roles.push(value.to_string());
            }
            true
        }
        _ => false,
    }
}

fn apply_comic_xml_credit(state: &mut ComicXmlParseState) {
    let Some(credit) = state.current_credit.take() else {
        return;
    };
    let Some(creator) = credit.creator else {
        return;
    };
    for role in credit.roles {
        match role.to_ascii_lowercase().as_str() {
            "writer" | "author" | "script" | "story" | "plot" => {
                set_comic_info_field(&mut state.metadata.writer, Some(&creator));
            }
            "artist" | "penciller" | "illustrator" => {
                set_comic_info_field(&mut state.metadata.penciller, Some(&creator));
            }
            _ => {}
        }
    }
}

fn apply_acbf_author(state: &mut ComicXmlParseState) {
    let Some(author) = state.current_acbf_author.take() else {
        return;
    };
    let name = author
        .nickname
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            let parts = [
                author.first_name.as_deref(),
                author.middle_name.as_deref(),
                author.last_name.as_deref(),
            ]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
            (!parts.is_empty()).then(|| parts.join(" "))
        });
    let Some(name) = name else {
        return;
    };
    match author
        .activity
        .as_deref()
        .unwrap_or("Writer")
        .to_ascii_lowercase()
        .as_str()
    {
        "writer" | "adapter" => set_comic_info_field(&mut state.metadata.writer, Some(&name)),
        "artist" | "penciller" => {
            set_comic_info_field(&mut state.metadata.penciller, Some(&name));
        }
        _ => {}
    }
}

fn set_comic_info_field(field: &mut Option<String>, value: Option<&str>) {
    if field.is_some() {
        return;
    }
    let Some(value) = value else {
        return;
    };
    let value = value.trim();
    if !value.is_empty() {
        *field = Some(value.to_string());
    }
}

fn set_comic_info_year_from_date(field: &mut Option<String>, value: &str) {
    if field.is_some() {
        return;
    }
    let year = value.trim().get(..4).filter(|year| is_year_token(year));
    set_comic_info_field(field, year);
}

fn xml_local_name(name: &[u8]) -> String {
    let local = name
        .iter()
        .position(|byte| *byte == b':')
        .map(|index| &name[index + 1..])
        .unwrap_or(name);
    String::from_utf8_lossy(local).to_string()
}

fn xml_attribute_value(
    event: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
    name: &str,
) -> Option<String> {
    event.attributes().flatten().find_map(|attribute| {
        (xml_local_name(attribute.key.as_ref()).eq_ignore_ascii_case(name))
            .then(|| {
                attribute
                    .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, decoder)
                    .ok()
            })
            .flatten()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}
