use super::super::{
    common::{present_str, push_metadata_field},
    metadata::{DocumentMetadata, render_document_preview},
};
use crate::{
    file_info::DocumentFormat,
    preview::{PreviewContent, PreviewVisual, PreviewVisualKind, PreviewVisualLayout},
};
use std::{
    collections::BTreeMap,
    collections::hash_map::DefaultHasher,
    env, fs,
    fs::File,
    hash::{Hash, Hasher},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

const PDB_HEADER_LEN: usize = 78;
const PDB_RECORD_ENTRY_LEN: usize = 8;
const PALMDOC_HEADER_LEN: usize = 16;
const MOBI_MAGIC: &[u8; 4] = b"MOBI";
const EXTH_MAGIC: &[u8; 4] = b"EXTH";
const MAX_KINDLE_RECORD0_BYTES: usize = 2 * 1024 * 1024;
const MAX_KINDLE_COVER_RECORD_BYTES: usize = 8 * 1024 * 1024;
const KINDLE_COVER_CACHE_VERSION: usize = 1;
const MOBI_TEXT_ENCODING_CP1252: u32 = 1252;
const MOBI_TEXT_ENCODING_UTF8: u32 = 65001;
const MOBI_EXTH_FLAG_PRESENT: u32 = 0x40;
const EXTH_COVER_OFFSET: u32 = 201;
const EXTH_THUMB_OFFSET: u32 = 202;

#[derive(Clone, Copy, Debug)]
enum KindleTextEncoding {
    Utf8,
    Windows1252,
}

#[derive(Debug)]
struct KindleDatabase {
    pdb_title: Option<String>,
    file_len: u64,
    modified: Option<SystemTime>,
    record_offsets: Vec<u64>,
}

#[derive(Debug, Default)]
struct KindlePreviewData {
    metadata: DocumentMetadata,
    visual: Option<PreviewVisual>,
}

#[derive(Debug, Default)]
struct KindleHeader {
    encrypted: bool,
    encryption: Option<String>,
    full_name: Option<String>,
    first_image_record_index: Option<u32>,
    exth_records: BTreeMap<u32, Vec<Vec<u8>>>,
    encoding: Option<KindleTextEncoding>,
}

#[derive(Clone, Copy, Debug)]
enum KindleCoverFormat {
    Gif,
    Jpeg,
    Png,
    Webp,
}

impl KindleCoverFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Gif => "gif",
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
        }
    }
}

pub(super) fn build_kindle_preview(path: &Path, format: DocumentFormat) -> Option<PreviewContent> {
    File::open(path).ok()?;
    let preview = parse_kindle_preview_data(path).unwrap_or_default();
    let mut content = render_document_preview(format, preview.metadata);
    if let Some(visual) = preview.visual {
        content = content.with_preview_visual(visual);
    }
    Some(content)
}

fn parse_kindle_preview_data(path: &Path) -> Option<KindlePreviewData> {
    let mut file = File::open(path).ok()?;
    let database = parse_kindle_database(&mut file)?;
    let record0 = read_record_bytes(&mut file, &database, 0, MAX_KINDLE_RECORD0_BYTES, true)?;
    let header = parse_kindle_header(&record0).unwrap_or_default();
    let metadata = render_kindle_metadata(&database, &header);
    let visual = extract_kindle_cover_visual(path, &mut file, &database, &header);

    Some(KindlePreviewData { metadata, visual })
}

fn parse_kindle_database(file: &mut File) -> Option<KindleDatabase> {
    let file_metadata = file.metadata().ok()?;
    let file_len = file_metadata.len();
    if file_len < PDB_HEADER_LEN as u64 {
        return None;
    }

    let mut header = [0_u8; PDB_HEADER_LEN];
    file.seek(SeekFrom::Start(0)).ok()?;
    file.read_exact(&mut header).ok()?;
    let record_count = usize::from(read_u16(&header, 76)?);
    if record_count == 0 {
        return None;
    }
    if (PDB_HEADER_LEN + record_count * PDB_RECORD_ENTRY_LEN) as u64 > file_len {
        return None;
    }

    let pdb_title = decode_pdb_name(&header[..32]);
    let mut record_offsets = Vec::with_capacity(record_count);
    for _ in 0..record_count {
        let mut record = [0_u8; PDB_RECORD_ENTRY_LEN];
        file.read_exact(&mut record).ok()?;
        let offset = read_u32(&record, 0)? as u64;
        if offset >= file_len {
            return None;
        }
        record_offsets.push(offset);
    }

    Some(KindleDatabase {
        pdb_title,
        file_len,
        modified: file_metadata.modified().ok(),
        record_offsets,
    })
}

fn read_record_bytes(
    file: &mut File,
    database: &KindleDatabase,
    index: usize,
    limit_bytes: usize,
    allow_truncation: bool,
) -> Option<Vec<u8>> {
    let start = *database.record_offsets.get(index)?;
    let end = database
        .record_offsets
        .get(index + 1)
        .copied()
        .unwrap_or(database.file_len);
    if end <= start || end > database.file_len {
        return None;
    }

    let record_len = end - start;
    if !allow_truncation && record_len > limit_bytes as u64 {
        return None;
    }
    let size = record_len.min(limit_bytes as u64) as usize;
    let mut bytes = vec![0_u8; size];
    file.seek(SeekFrom::Start(start)).ok()?;
    file.read_exact(&mut bytes).ok()?;
    (!bytes.is_empty()).then_some(bytes)
}

fn render_kindle_metadata(database: &KindleDatabase, header: &KindleHeader) -> DocumentMetadata {
    let encoding = header.encoding.unwrap_or(KindleTextEncoding::Windows1252);
    let mut metadata = DocumentMetadata {
        title: first_exth_text(header, encoding, &[503])
            .or_else(|| header.full_name.clone())
            .or_else(|| database.pdb_title.clone()),
        author: joined_exth_text(header, encoding, 100, ", "),
        subject: joined_exth_text(header, encoding, 105, ", "),
        modified: first_exth_text(header, encoding, &[502])
            .map(|updated| normalize_kindle_date(&updated)),
        application: creator_application(header),
        ..DocumentMetadata::default()
    };

    push_metadata_field(
        &mut metadata,
        "Publisher",
        first_exth_text(header, encoding, &[101]),
    );
    push_metadata_field(
        &mut metadata,
        "Language",
        first_exth_text(header, encoding, &[524]),
    );
    push_metadata_field(
        &mut metadata,
        "Published",
        first_exth_text(header, encoding, &[106]).map(|value| normalize_kindle_date(&value)),
    );
    push_metadata_field(
        &mut metadata,
        "ISBN",
        first_exth_text(header, encoding, &[104]),
    );
    push_metadata_field(
        &mut metadata,
        "ASIN",
        first_exth_text(header, encoding, &[113, 504]),
    );
    push_metadata_field(
        &mut metadata,
        "Description",
        first_exth_text(header, encoding, &[103]),
    );
    push_metadata_field(
        &mut metadata,
        "Rights",
        first_exth_text(header, encoding, &[109]),
    );
    push_metadata_field(
        &mut metadata,
        "Kindle Type",
        first_exth_text(header, encoding, &[501]),
    );
    push_metadata_field(&mut metadata, "Encryption", header.encryption.clone());

    metadata
}

fn extract_kindle_cover_visual(
    path: &Path,
    file: &mut File,
    database: &KindleDatabase,
    header: &KindleHeader,
) -> Option<PreviewVisual> {
    if header.encrypted {
        return None;
    }

    let first_image_record_index = usize::try_from(header.first_image_record_index?).ok()?;
    for relative_cover_index in kindle_cover_record_offsets(header) {
        let Some(cover_record_index) = usize::try_from(relative_cover_index)
            .ok()
            .and_then(|index| first_image_record_index.checked_add(index))
        else {
            continue;
        };
        if cover_record_index >= database.record_offsets.len() {
            continue;
        }
        if let Some(visual) =
            extract_kindle_cover_record_visual(path, file, database, cover_record_index)
        {
            return Some(visual);
        }
    }

    None
}

fn extract_kindle_cover_record_visual(
    path: &Path,
    file: &mut File,
    database: &KindleDatabase,
    cover_record_index: usize,
) -> Option<PreviewVisual> {
    let cache_path =
        if let Some(cache_path) = cached_kindle_cover_path(path, database, cover_record_index) {
            cache_path
        } else {
            let bytes = read_record_bytes(
                file,
                database,
                cover_record_index,
                MAX_KINDLE_COVER_RECORD_BYTES,
                false,
            )?;
            let format = sniff_kindle_cover_format(&bytes)?;
            let cache_path =
                kindle_cover_cache_path(path, database, cover_record_index, format.extension())?;
            if !cache_path.exists() {
                write_bytes_atomically(&cache_path, &bytes)?;
            }
            cache_path
        };

    kindle_cover_visual_from_path(cache_path)
}

fn cached_kindle_cover_path(
    path: &Path,
    database: &KindleDatabase,
    record_index: usize,
) -> Option<PathBuf> {
    for extension in ["jpg", "png", "gif", "webp"] {
        let cache_path = kindle_cover_cache_path(path, database, record_index, extension)?;
        if cache_path.exists() {
            return Some(cache_path);
        }
    }
    None
}

fn kindle_cover_record_offsets(header: &KindleHeader) -> Vec<u32> {
    let mut offsets = Vec::new();
    for kind in [EXTH_COVER_OFFSET, EXTH_THUMB_OFFSET] {
        if let Some(offset) = first_exth_integer(header, &[kind])
            && !offsets.contains(&offset)
        {
            offsets.push(offset);
        }
    }
    offsets
}

fn parse_kindle_header(record0: &[u8]) -> Option<KindleHeader> {
    if record0.len() < PALMDOC_HEADER_LEN + 8
        || record0.get(PALMDOC_HEADER_LEN..PALMDOC_HEADER_LEN + 4)? != MOBI_MAGIC
    {
        return None;
    }

    let encryption = read_u16(record0, 12).unwrap_or(0);
    let mut header = KindleHeader {
        encrypted: encryption != 0,
        encryption: encryption_label(encryption),
        ..KindleHeader::default()
    };

    let mobi_header_len = read_u32(record0, 20)? as usize;
    if mobi_header_len < 24 {
        return Some(header);
    }
    let mobi_header_end = PALMDOC_HEADER_LEN.checked_add(mobi_header_len)?;
    if mobi_header_end > record0.len() {
        return Some(header);
    }

    header.encoding = read_u32(record0, 28).map(kindle_text_encoding);
    if mobi_header_len >= 96 {
        header.first_image_record_index = read_u32(record0, 108);
    }
    let encoding = header.encoding.unwrap_or(KindleTextEncoding::Windows1252);

    if mobi_header_len >= 76
        && let (Some(offset), Some(length)) = (read_u32(record0, 84), read_u32(record0, 88))
    {
        header.full_name = decode_record0_text(record0, offset as usize, length as usize, encoding);
    }

    let exth_flags = if mobi_header_len >= 116 {
        read_u32(record0, 128).unwrap_or(0)
    } else {
        0
    };
    if exth_flags & MOBI_EXTH_FLAG_PRESENT != 0 {
        header.exth_records = parse_exth_records(record0, mobi_header_end);
    }

    Some(header)
}

fn parse_exth_records(record0: &[u8], exth_offset: usize) -> BTreeMap<u32, Vec<Vec<u8>>> {
    let mut records = BTreeMap::<u32, Vec<Vec<u8>>>::new();
    if exth_offset
        .checked_add(12)
        .is_none_or(|end| end > record0.len())
        || record0.get(exth_offset..exth_offset + 4) != Some(EXTH_MAGIC)
    {
        return records;
    }

    let Some(exth_len) = read_u32(record0, exth_offset + 4).map(|value| value as usize) else {
        return records;
    };
    let Some(record_count) = read_u32(record0, exth_offset + 8).map(|value| value as usize) else {
        return records;
    };
    let Some(exth_end) = exth_offset.checked_add(exth_len) else {
        return records;
    };
    if exth_len < 12 || exth_end > record0.len() {
        return records;
    }

    let mut cursor = exth_offset + 12;
    for _ in 0..record_count {
        if cursor.checked_add(8).is_none_or(|end| end > exth_end) {
            break;
        }
        let Some(kind) = read_u32(record0, cursor) else {
            break;
        };
        let Some(len) = read_u32(record0, cursor + 4).map(|value| value as usize) else {
            break;
        };
        if len < 8 || cursor.checked_add(len).is_none_or(|end| end > exth_end) {
            break;
        }
        let value = record0[cursor + 8..cursor + len].to_vec();
        records.entry(kind).or_default().push(value);
        cursor += len;
    }

    records
}

fn first_exth_text(
    header: &KindleHeader,
    encoding: KindleTextEncoding,
    kinds: &[u32],
) -> Option<String> {
    kinds
        .iter()
        .filter_map(|kind| header.exth_records.get(kind))
        .flat_map(|values| values.iter())
        .find_map(|value| decode_metadata_text(value, encoding))
}

fn joined_exth_text(
    header: &KindleHeader,
    encoding: KindleTextEncoding,
    kind: u32,
    separator: &str,
) -> Option<String> {
    let values = header.exth_records.get(&kind)?;
    let mut decoded = Vec::<String>::new();
    for value in values {
        let Some(value) = decode_metadata_text(value, encoding) else {
            continue;
        };
        if decoded.iter().all(|existing| existing != &value) {
            decoded.push(value);
        }
    }
    (!decoded.is_empty()).then(|| decoded.join(separator))
}

fn first_exth_integer(header: &KindleHeader, kinds: &[u32]) -> Option<u32> {
    kinds
        .iter()
        .filter_map(|kind| header.exth_records.get(kind))
        .flat_map(|values| values.iter())
        .find_map(|value| exth_integer(value))
}

fn creator_application(header: &KindleHeader) -> Option<String> {
    let software = header
        .exth_records
        .get(&204)
        .and_then(|values| values.first())
        .and_then(|value| exth_integer(value))
        .and_then(creator_software_label);
    let major = header
        .exth_records
        .get(&205)
        .and_then(|values| values.first())
        .and_then(|value| exth_integer(value));
    let minor = header
        .exth_records
        .get(&206)
        .and_then(|values| values.first())
        .and_then(|value| exth_integer(value));
    let build = header
        .exth_records
        .get(&207)
        .and_then(|values| values.first())
        .and_then(|value| exth_integer(value));

    let software = software?;
    let mut label = software.to_string();
    if let (Some(major), Some(minor)) = (major, minor) {
        label.push_str(&format!(" {major}.{minor}"));
    }
    if let Some(build) = build {
        label.push_str(&format!(" build {build}"));
    }
    Some(label)
}

fn creator_software_label(value: u32) -> Option<&'static str> {
    match value {
        1 => Some("mobigen"),
        2 => Some("Mobipocket Creator"),
        200..=202 => Some("KindleGen"),
        _ => None,
    }
}

fn exth_integer(bytes: &[u8]) -> Option<u32> {
    if bytes.len() == 1 {
        return Some(bytes[0] as u32);
    }
    if bytes.len() == 2 {
        return Some(u16::from_be_bytes([bytes[0], bytes[1]]) as u32);
    }
    if bytes.len() == 4 {
        return Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
    }
    None
}

fn normalize_kindle_date(value: &str) -> String {
    present_str(value.trim(), "Created").unwrap_or_else(|| value.trim().to_string())
}

fn encryption_label(value: u16) -> Option<String> {
    match value {
        0 => None,
        1 => Some("Old Mobipocket encryption".to_string()),
        2 => Some("Mobipocket encryption".to_string()),
        _ => Some(format!("Unknown ({value})")),
    }
}

fn kindle_text_encoding(value: u32) -> KindleTextEncoding {
    match value {
        MOBI_TEXT_ENCODING_UTF8 => KindleTextEncoding::Utf8,
        MOBI_TEXT_ENCODING_CP1252 => KindleTextEncoding::Windows1252,
        _ => KindleTextEncoding::Windows1252,
    }
}

fn decode_record0_text(
    record0: &[u8],
    offset: usize,
    length: usize,
    encoding: KindleTextEncoding,
) -> Option<String> {
    let end = offset.checked_add(length)?;
    let bytes = record0.get(offset..end)?;
    decode_metadata_text(bytes, encoding)
}

fn decode_metadata_text(bytes: &[u8], encoding: KindleTextEncoding) -> Option<String> {
    let bytes = trim_metadata_bytes(bytes);
    if bytes.is_empty() {
        return None;
    }
    let value = match encoding {
        KindleTextEncoding::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
        KindleTextEncoding::Windows1252 => decode_windows_1252(bytes),
    };
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn decode_pdb_name(bytes: &[u8]) -> Option<String> {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    decode_metadata_text(&bytes[..end], KindleTextEncoding::Windows1252)
}

fn trim_metadata_bytes(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && (bytes[end - 1].is_ascii_whitespace() || bytes[end - 1] == 0) {
        end -= 1;
    }
    &bytes[start..end]
}

fn decode_windows_1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| match *byte {
            0x80 => '\u{20AC}',
            0x82 => '\u{201A}',
            0x83 => '\u{0192}',
            0x84 => '\u{201E}',
            0x85 => '\u{2026}',
            0x86 => '\u{2020}',
            0x87 => '\u{2021}',
            0x88 => '\u{02C6}',
            0x89 => '\u{2030}',
            0x8A => '\u{0160}',
            0x8B => '\u{2039}',
            0x8C => '\u{0152}',
            0x8E => '\u{017D}',
            0x91 => '\u{2018}',
            0x92 => '\u{2019}',
            0x93 => '\u{201C}',
            0x94 => '\u{201D}',
            0x95 => '\u{2022}',
            0x96 => '\u{2013}',
            0x97 => '\u{2014}',
            0x98 => '\u{02DC}',
            0x99 => '\u{2122}',
            0x9A => '\u{0161}',
            0x9B => '\u{203A}',
            0x9C => '\u{0153}',
            0x9E => '\u{017E}',
            0x9F => '\u{0178}',
            value => char::from(value),
        })
        .collect()
}

fn sniff_kindle_cover_format(bytes: &[u8]) -> Option<KindleCoverFormat> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some(KindleCoverFormat::Png);
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(KindleCoverFormat::Jpeg);
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(KindleCoverFormat::Gif);
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some(KindleCoverFormat::Webp);
    }
    None
}

fn kindle_cover_visual_from_path(path: PathBuf) -> Option<PreviewVisual> {
    let metadata = fs::metadata(&path).ok()?;
    Some(PreviewVisual {
        kind: PreviewVisualKind::Cover,
        layout: PreviewVisualLayout::LargeInline,
        path,
        size: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn kindle_cover_cache_path(
    path: &Path,
    database: &KindleDatabase,
    record_index: usize,
    extension: &str,
) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    KINDLE_COVER_CACHE_VERSION.hash(&mut hasher);
    path.hash(&mut hasher);
    database.file_len.hash(&mut hasher);
    database
        .modified
        .and_then(system_time_key)
        .hash(&mut hasher);
    record_index.hash(&mut hasher);
    let cache_dir = kindle_cover_cache_dir()?;
    Some(cache_dir.join(format!("cover-{:016x}.{extension}", hasher.finish())))
}

fn kindle_cover_cache_dir() -> Option<PathBuf> {
    let cache_dir =
        env::temp_dir().join(format!("elio-kindle-cover-v{KINDLE_COVER_CACHE_VERSION}"));
    fs::create_dir_all(&cache_dir).ok()?;
    Some(cache_dir)
}

fn write_bytes_atomically(path: &Path, bytes: &[u8]) -> Option<()> {
    let parent = path.parent()?;
    fs::create_dir_all(parent).ok()?;

    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temp_name = format!(
        ".{}.tmp-{}-{}",
        path.file_name()?.to_string_lossy(),
        std::process::id(),
        unique
    );
    let temp_path = parent.join(temp_name);

    let mut file = File::create(&temp_path).ok()?;
    file.write_all(bytes).ok()?;
    file.sync_all().ok()?;
    match fs::rename(&temp_path, path) {
        Ok(()) => Some(()),
        Err(_) if path.exists() => {
            let _ = fs::remove_file(&temp_path);
            Some(())
        }
        Err(_) => {
            let _ = fs::remove_file(&temp_path);
            None
        }
    }
}

fn system_time_key(time: SystemTime) -> Option<(u64, u32)> {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| (duration.as_secs(), duration.subsec_nanos()))
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let bytes = bytes.get(offset..offset + 2)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let bytes = bytes.get(offset..offset + 4)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}
