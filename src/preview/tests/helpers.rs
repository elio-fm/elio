use super::*;
use crate::app::{Entry, EntryKind};
use flate2::{Compression, write::GzEncoder};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use ratatui::style::Color;
use ratatui::text::Line;
use std::{
    fs,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use tar::{Builder as TarBuilder, Header as TarHeader};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

pub(super) fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-preview-{label}-{unique}"))
}

pub(super) fn file_entry(path: PathBuf) -> Entry {
    Entry {
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        name_key: path.file_name().unwrap().to_string_lossy().to_lowercase(),
        path,
        kind: EntryKind::File,
        size: 0,
        modified: None,
        readonly: false,
    }
}

pub(super) fn directory_entry(path: PathBuf) -> Entry {
    Entry {
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        name_key: path.file_name().unwrap().to_string_lossy().to_lowercase(),
        path,
        kind: EntryKind::Directory,
        size: 0,
        modified: None,
        readonly: false,
    }
}

pub(super) fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
}

pub(super) fn span_color<'a>(line: &'a Line<'a>, token: &str) -> Option<Color> {
    line.spans
        .iter()
        .find(|span| span.content.contains(token))
        .and_then(|span| span.style.fg)
}

pub(super) fn line_has_color(line: &Line<'_>, color: Color) -> bool {
    line.spans.iter().any(|span| span.style.fg == Some(color))
}

pub(super) fn bencode_bytes(value: &[u8]) -> Vec<u8> {
    let mut encoded = format!("{}:", value.len()).into_bytes();
    encoded.extend_from_slice(value);
    encoded
}

pub(super) fn bencode_str(value: &str) -> Vec<u8> {
    bencode_bytes(value.as_bytes())
}

pub(super) fn bencode_int(value: i64) -> Vec<u8> {
    format!("i{value}e").into_bytes()
}

pub(super) fn bencode_list(values: Vec<Vec<u8>>) -> Vec<u8> {
    let mut encoded = vec![b'l'];
    for value in values {
        encoded.extend(value);
    }
    encoded.push(b'e');
    encoded
}

pub(super) fn bencode_dict(entries: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let mut encoded = vec![b'd'];
    for (key, value) in entries {
        encoded.extend(bencode_str(key));
        encoded.extend(value);
    }
    encoded.push(b'e');
    encoded
}

pub(super) fn write_iso_field(bytes: &mut [u8], start: usize, end: usize, value: &str) {
    let field = &mut bytes[start..end];
    field.fill(b' ');
    let copy_len = value.len().min(field.len());
    field[..copy_len].copy_from_slice(&value.as_bytes()[..copy_len]);
}

pub(super) fn put_iso_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn put_iso_u16_le(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn sample_iso_descriptors() -> Vec<u8> {
    let mut bytes = vec![0u8; (ISO_DESCRIPTOR_START_SECTOR + 3) * ISO_SECTOR_SIZE];
    let start = ISO_DESCRIPTOR_START_SECTOR * ISO_SECTOR_SIZE;

    let boot = &mut bytes[start..start + ISO_SECTOR_SIZE];
    boot[0] = 0;
    boot[1..6].copy_from_slice(b"CD001");
    boot[6] = 1;
    write_iso_field(boot, 7, 39, ISO_BOOT_SYSTEM_ID);

    let primary = &mut bytes[start + ISO_SECTOR_SIZE..start + ISO_SECTOR_SIZE * 2];
    primary[0] = 1;
    primary[1..6].copy_from_slice(b"CD001");
    primary[6] = 1;
    write_iso_field(primary, 8, 40, "ELIO_SYS");
    write_iso_field(primary, 40, 72, "ELIO_INSTALL");
    put_iso_u32_le(primary, 80, 640);
    put_iso_u16_le(primary, 128, ISO_SECTOR_SIZE as u16);
    write_iso_field(primary, 318, 446, "Elio Publisher");
    write_iso_field(primary, 446, 574, "Elio Builder");
    write_iso_field(primary, 574, 702, "Elio Image Tool");
    write_iso_field(primary, 813, 830, "20260311090000000");
    write_iso_field(primary, 830, 847, "20260311101500000");
    write_iso_field(primary, 864, 881, "20260312000000000");

    let terminator = &mut bytes[start + ISO_SECTOR_SIZE * 2..start + ISO_SECTOR_SIZE * 3];
    terminator[0] = 255;
    terminator[1..6].copy_from_slice(b"CD001");
    terminator[6] = 1;
    bytes
}

pub(super) fn write_zip_entries(path: &Path, entries: &[(&str, &str)]) {
    let file = File::create(path).expect("failed to create zip");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for (name, contents) in entries {
        zip.start_file(name, options)
            .expect("failed to start zip entry");
        zip.write_all(contents.as_bytes())
            .expect("failed to write zip entry");
    }

    zip.finish().expect("failed to finish zip");
}

pub(super) fn write_zip_binary_entries(path: &Path, entries: &[(&str, &[u8])]) {
    let file = File::create(path).expect("failed to create zip");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for (name, contents) in entries {
        zip.start_file(name, options)
            .expect("failed to start zip entry");
        zip.write_all(contents).expect("failed to write zip entry");
    }

    zip.finish().expect("failed to finish zip");
}

pub(super) enum DocTestPropertyValue<'a> {
    Count(u32),
    Text(&'a str),
    Timestamp(u64),
}

pub(super) fn write_doc_summary_information(
    path: &Path,
    properties: &[(u32, DocTestPropertyValue<'_>)],
) {
    const DOC_SUMMARY_INFORMATION_STREAM: &str = "/\u{5}SummaryInformation";
    const VT_LPWSTR: u16 = 0x001F;
    const VT_FILETIME: u16 = 0x0040;
    const VT_UI4: u16 = 0x0013;

    fn encode_doc_property_value(value: &DocTestPropertyValue<'_>) -> Vec<u8> {
        match value {
            DocTestPropertyValue::Count(count) => {
                let mut bytes = Vec::with_capacity(8);
                bytes.extend_from_slice(&VT_UI4.to_le_bytes());
                bytes.extend_from_slice(&0u16.to_le_bytes());
                bytes.extend_from_slice(&count.to_le_bytes());
                bytes
            }
            DocTestPropertyValue::Text(text) => {
                let mut bytes = Vec::new();
                let mut units = text.encode_utf16().collect::<Vec<_>>();
                units.push(0);
                bytes.extend_from_slice(&VT_LPWSTR.to_le_bytes());
                bytes.extend_from_slice(&0u16.to_le_bytes());
                bytes.extend_from_slice(&(units.len() as u32).to_le_bytes());
                for unit in units {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
                bytes
            }
            DocTestPropertyValue::Timestamp(unix_seconds) => {
                const WINDOWS_TICKS_PER_SECOND: u64 = 10_000_000;
                const WINDOWS_TO_UNIX_EPOCH_SECONDS: u64 = 11_644_473_600;

                let filetime =
                    (unix_seconds + WINDOWS_TO_UNIX_EPOCH_SECONDS) * WINDOWS_TICKS_PER_SECOND;
                let mut bytes = Vec::with_capacity(12);
                bytes.extend_from_slice(&VT_FILETIME.to_le_bytes());
                bytes.extend_from_slice(&0u16.to_le_bytes());
                bytes.extend_from_slice(&filetime.to_le_bytes());
                bytes
            }
        }
    }

    let section_offset = 48usize;
    let table_len = 8 + properties.len() * 8;
    let mut section = vec![0; table_len];
    section[4..8].copy_from_slice(&(properties.len() as u32).to_le_bytes());
    let mut values = Vec::new();

    for (index, (property_id, value)) in properties.iter().enumerate() {
        let encoded = encode_doc_property_value(value);
        let entry_offset = 8 + index * 8;
        section[entry_offset..entry_offset + 4].copy_from_slice(&property_id.to_le_bytes());
        section[entry_offset + 4..entry_offset + 8]
            .copy_from_slice(&((table_len + values.len()) as u32).to_le_bytes());
        values.extend_from_slice(&encoded);
    }

    let mut bytes = vec![0; section_offset];
    bytes[28..32].copy_from_slice(&1u32.to_le_bytes());
    bytes[44..48].copy_from_slice(&(section_offset as u32).to_le_bytes());
    bytes.extend_from_slice(&section);
    bytes.extend_from_slice(&values);

    let mut compound = cfb::create(path).expect("failed to create compound document");
    let mut stream = compound
        .create_stream(DOC_SUMMARY_INFORMATION_STREAM)
        .expect("failed to create summary information stream");
    stream
        .write_all(&bytes)
        .expect("failed to write summary information stream");
}

pub(super) fn write_test_raster_image(
    path: &Path,
    format: ImageFormat,
    width_px: u32,
    height_px: u32,
) {
    let mut image = RgbaImage::new(width_px, height_px);
    for pixel in image.pixels_mut() {
        *pixel = Rgba([32, 128, 224, 255]);
    }

    DynamicImage::ImageRgba8(image)
        .save_with_format(path, format)
        .expect("failed to write raster test image");
}

pub(super) fn write_tar_entries(path: &Path, entries: &[(&str, &str)]) {
    let file = File::create(path).expect("failed to create tar");
    let mut builder = TarBuilder::new(file);
    append_tar_entries(&mut builder, entries);
    builder.finish().expect("failed to finish tar");
}

pub(super) fn write_tar_gz_entries(path: &Path, entries: &[(&str, &str)]) {
    let file = File::create(path).expect("failed to create tar.gz");
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = TarBuilder::new(encoder);
    append_tar_entries(&mut builder, entries);
    builder.finish().expect("failed to finish tar.gz");
}

pub(super) fn append_tar_entries<W: Write>(builder: &mut TarBuilder<W>, entries: &[(&str, &str)]) {
    for (name, contents) in entries {
        if let Some(parent) = Path::new(name).parent() {
            append_tar_directories(builder, parent);
        }

        let mut header = TarHeader::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o644);
        header.set_size(contents.len() as u64);
        header.set_cksum();
        builder
            .append_data(&mut header, *name, contents.as_bytes())
            .expect("failed to append tar entry");
    }
}

pub(super) fn append_tar_directories<W: Write>(builder: &mut TarBuilder<W>, path: &Path) {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        let mut header = TarHeader::new_gnu();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_mode(0o755);
        header.set_size(0);
        header.set_cksum();
        let _ = builder.append_data(&mut header, &current, std::io::empty());
    }
}

pub(super) fn write_xz_compressed_file(path: &Path, contents: &[u8]) -> bool {
    let source = path.with_extension("");
    fs::write(&source, contents).expect("failed to write xz staging file");

    let created = Command::new("xz").arg("-zk").arg(&source).status();
    let _ = fs::remove_file(&source);
    created.as_ref().is_ok_and(|status| status.success()) && path.exists()
}

pub(super) fn put_u16_le(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn put_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn put_u32_be(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

pub(super) fn put_u64_le(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn sample_pe_exe_bytes() -> Vec<u8> {
    let mut bytes = vec![0u8; 0x200];
    bytes[0..2].copy_from_slice(b"MZ");
    put_u32_le(&mut bytes, 0x3c, 0x80);

    let pe = 0x80;
    bytes[pe..pe + 4].copy_from_slice(b"PE\0\0");
    put_u16_le(&mut bytes, pe + 4, 0x8664);
    put_u16_le(&mut bytes, pe + 6, 3);
    put_u16_le(&mut bytes, pe + 20, 0x00f0);
    put_u16_le(&mut bytes, pe + 22, 0x0022);

    let optional = pe + 24;
    put_u16_le(&mut bytes, optional, 0x20b);
    put_u32_le(&mut bytes, optional + 16, 0x1230);
    put_u16_le(&mut bytes, optional + 88, 3);
    bytes
}

pub(super) fn sample_elf_shared_object_bytes() -> Vec<u8> {
    let mut bytes = vec![0u8; 64];
    bytes[0..4].copy_from_slice(b"\x7FELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 3;
    put_u16_le(&mut bytes, 16, 3);
    put_u16_le(&mut bytes, 18, 0x00b7);
    put_u64_le(&mut bytes, 24, 0x401000);
    put_u16_le(&mut bytes, 56, 8);
    put_u16_le(&mut bytes, 60, 18);
    bytes
}

pub(super) fn sample_macho_dylib_bytes() -> Vec<u8> {
    let mut bytes = vec![0u8; 32];
    bytes[0..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
    put_u32_le(&mut bytes, 4, 0x0100000c);
    put_u32_le(&mut bytes, 12, 6);
    put_u32_le(&mut bytes, 16, 12);
    bytes
}

pub(super) fn sample_dos_mz_bytes() -> Vec<u8> {
    let mut bytes = vec![0u8; 64];
    bytes[0..2].copy_from_slice(b"MZ");
    bytes
}

pub(super) fn sample_macho_fat_bytes() -> Vec<u8> {
    let mut bytes = vec![0u8; 48];
    bytes[0..4].copy_from_slice(&[0xca, 0xfe, 0xba, 0xbe]);
    put_u32_be(&mut bytes, 4, 2);
    put_u32_be(&mut bytes, 8, 7);
    put_u32_be(&mut bytes, 28, 0x0100000c);
    bytes
}

pub(super) fn sample_pdf_bytes() -> Vec<u8> {
    let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 144] /Resources << >> /Contents 4 0 R >>"
                .to_string(),
            "<< /Length 0 >>\nstream\n\nendstream".to_string(),
            "<< /Title (Quarterly Report) /Author (Regueiro) /Creator (Elio) /Producer (Elio Test Suite) /CreationDate (D:20260311120000Z) /ModDate (D:20260311123000Z) >>".to_string(),
        ];

    let mut bytes = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }

    let xref_offset = bytes.len();
    bytes.extend(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    bytes.extend(b"0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );
    bytes
}
