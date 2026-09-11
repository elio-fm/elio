use super::super::{
    metadata_formatting::{present_str, push_metadata_field},
    metadata_preview::DocumentMetadata,
};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    time::SystemTime,
};

const PDB_HEADER_LEN: usize = 78;
const PDB_RECORD_ENTRY_LEN: usize = 8;
const PALMDOC_HEADER_LEN: usize = 16;
const MOBI_MAGIC: &[u8; 4] = b"MOBI";
const EXTH_MAGIC: &[u8; 4] = b"EXTH";
pub(super) const MAX_KINDLE_RECORD0_BYTES: usize = 2 * 1024 * 1024;
const MOBI_TEXT_ENCODING_CP1252: u32 = 1252;
const MOBI_TEXT_ENCODING_UTF8: u32 = 65001;
const MOBI_EXTH_FLAG_PRESENT: u32 = 0x40;

#[derive(Clone, Copy, Debug)]
enum KindleTextEncoding {
    Utf8,
    Windows1252,
}

#[derive(Debug)]
pub(super) struct KindleDatabase {
    pdb_title: Option<String>,
    pub(super) file_len: u64,
    pub(super) modified: Option<SystemTime>,
    pub(super) record_offsets: Vec<u64>,
}

#[derive(Debug, Default)]
pub(super) struct KindleHeader {
    pub(super) encrypted: bool,
    encryption: Option<String>,
    full_name: Option<String>,
    pub(super) first_image_record_index: Option<u32>,
    pub(super) exth_records: BTreeMap<u32, Vec<Vec<u8>>>,
    encoding: Option<KindleTextEncoding>,
}

pub(super) fn parse_kindle_database(file: &mut File) -> Option<KindleDatabase> {
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

pub(super) fn read_record_bytes(
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

pub(super) fn render_kindle_metadata(
    database: &KindleDatabase,
    header: &KindleHeader,
) -> DocumentMetadata {
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

pub(super) fn parse_kindle_header(record0: &[u8]) -> Option<KindleHeader> {
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

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let bytes = bytes.get(offset..offset + 2)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let bytes = bytes.get(offset..offset + 4)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}
