use super::*;

fn write_synthetic_kindle(path: &std::path::Path, full_name: &str, exth_records: &[(u32, &[u8])]) {
    write_synthetic_kindle_with_resources(path, full_name, exth_records, &[]);
}

fn write_synthetic_kindle_with_resources(
    path: &std::path::Path,
    full_name: &str,
    exth_records: &[(u32, &[u8])],
    resource_records: &[&[u8]],
) {
    write_synthetic_kindle_with_options(path, full_name, exth_records, resource_records, 0);
}

fn write_synthetic_kindle_with_options(
    path: &std::path::Path,
    full_name: &str,
    exth_records: &[(u32, &[u8])],
    resource_records: &[&[u8]],
    encryption: u16,
) {
    let mut record0 = vec![0_u8; 16];
    write_be_u16(&mut record0, 0, 2);
    write_be_u32(&mut record0, 4, 12_345);
    write_be_u16(&mut record0, 8, 2);
    write_be_u16(&mut record0, 10, 4096);
    write_be_u16(&mut record0, 12, encryption);

    let mut mobi_header = vec![0_u8; 256];
    mobi_header[0..4].copy_from_slice(b"MOBI");
    write_be_u32(&mut mobi_header, 4, 256);
    write_be_u32(&mut mobi_header, 8, 2);
    write_be_u32(&mut mobi_header, 12, 65_001);
    write_be_u32(&mut mobi_header, 20, 6);

    let exth = build_exth(exth_records);
    let full_name_offset = 16 + mobi_header.len() + exth.len();
    write_be_u32(&mut mobi_header, 68, full_name_offset as u32);
    write_be_u32(&mut mobi_header, 72, full_name.len() as u32);
    if !resource_records.is_empty() {
        write_be_u32(&mut mobi_header, 92, 1);
    }
    if !exth_records.is_empty() {
        write_be_u32(&mut mobi_header, 112, 0x40);
    }

    record0.extend_from_slice(&mobi_header);
    record0.extend_from_slice(&exth);
    record0.extend_from_slice(full_name.as_bytes());
    record0.extend_from_slice(&[0, 0]);
    while !record0.len().is_multiple_of(4) {
        record0.push(0);
    }

    let mut records = vec![record0];
    records.extend(resource_records.iter().map(|record| record.to_vec()));
    records.push(b"EOF".to_vec());

    let record_count = records.len() as u16;
    let record0_offset = 78 + usize::from(record_count) * 8;
    let mut bytes = vec![0_u8; 78];
    bytes[..13].copy_from_slice(b"Fixture Book\0");
    bytes[60..64].copy_from_slice(b"BOOK");
    bytes[64..68].copy_from_slice(b"MOBI");
    write_be_u16(&mut bytes, 76, record_count);
    let mut offset = record0_offset;
    for record in &records {
        write_record_entry(&mut bytes, offset as u32);
        offset += record.len();
    }
    for record in records {
        bytes.extend_from_slice(&record);
    }

    fs::write(path, bytes).expect("failed to write synthetic kindle file");
}

fn raster_image_bytes(
    root: &std::path::Path,
    format: ImageFormat,
    width_px: u32,
    height_px: u32,
) -> Vec<u8> {
    let extension = match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        _ => "img",
    };
    let path = root.join(format!("cover.{extension}"));
    write_test_raster_image(&path, format, width_px, height_px);
    fs::read(path).expect("failed to read raster test image")
}

fn build_exth(records: &[(u32, &[u8])]) -> Vec<u8> {
    if records.is_empty() {
        return Vec::new();
    }

    let mut exth = Vec::new();
    exth.extend_from_slice(b"EXTH");
    exth.extend_from_slice(&[0; 4]);
    exth.extend_from_slice(&(records.len() as u32).to_be_bytes());
    for (kind, value) in records {
        exth.extend_from_slice(&kind.to_be_bytes());
        exth.extend_from_slice(&((8 + value.len()) as u32).to_be_bytes());
        exth.extend_from_slice(value);
    }
    let exth_len = exth.len() as u32;
    exth[4..8].copy_from_slice(&exth_len.to_be_bytes());
    while !exth.len().is_multiple_of(4) {
        exth.push(0);
    }
    exth
}

fn write_record_entry(bytes: &mut Vec<u8>, offset: u32) {
    bytes.extend_from_slice(&offset.to_be_bytes());
    bytes.extend_from_slice(&[0, 0, 0, 0]);
}

fn write_be_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn write_be_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

mod cover_images;
mod kindle_metadata;
mod kindle_preview;
