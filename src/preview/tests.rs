use super::*;
use crate::{
    app::{Entry, EntryKind},
    ui::theme,
};
use flate2::{Compression, write::GzEncoder};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use ratatui::style::{Color, Modifier};
use ratatui::text::Line;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Barrier},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tar::{Builder as TarBuilder, Header as TarHeader};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-preview-{label}-{unique}"))
}

fn file_entry(path: PathBuf) -> Entry {
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

fn directory_entry(path: PathBuf) -> Entry {
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

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
}

fn span_color<'a>(line: &'a Line<'a>, token: &str) -> Option<Color> {
    line.spans
        .iter()
        .find(|span| span.content.contains(token))
        .and_then(|span| span.style.fg)
}

fn line_has_color(line: &Line<'_>, color: Color) -> bool {
    line.spans.iter().any(|span| span.style.fg == Some(color))
}

fn bencode_bytes(value: &[u8]) -> Vec<u8> {
    let mut encoded = format!("{}:", value.len()).into_bytes();
    encoded.extend_from_slice(value);
    encoded
}

fn bencode_str(value: &str) -> Vec<u8> {
    bencode_bytes(value.as_bytes())
}

fn bencode_int(value: i64) -> Vec<u8> {
    format!("i{value}e").into_bytes()
}

fn bencode_list(values: Vec<Vec<u8>>) -> Vec<u8> {
    let mut encoded = vec![b'l'];
    for value in values {
        encoded.extend(value);
    }
    encoded.push(b'e');
    encoded
}

fn bencode_dict(entries: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let mut encoded = vec![b'd'];
    for (key, value) in entries {
        encoded.extend(bencode_str(key));
        encoded.extend(value);
    }
    encoded.push(b'e');
    encoded
}

fn write_iso_field(bytes: &mut [u8], start: usize, end: usize, value: &str) {
    let field = &mut bytes[start..end];
    field.fill(b' ');
    let copy_len = value.len().min(field.len());
    field[..copy_len].copy_from_slice(&value.as_bytes()[..copy_len]);
}

fn put_iso_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_iso_u16_le(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn sample_iso_descriptors() -> Vec<u8> {
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

fn write_zip_entries(path: &Path, entries: &[(&str, &str)]) {
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

fn write_zip_binary_entries(path: &Path, entries: &[(&str, &[u8])]) {
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

fn write_test_raster_image(path: &Path, format: ImageFormat, width_px: u32, height_px: u32) {
    let mut image = RgbaImage::new(width_px, height_px);
    for pixel in image.pixels_mut() {
        *pixel = Rgba([32, 128, 224, 255]);
    }

    DynamicImage::ImageRgba8(image)
        .save_with_format(path, format)
        .expect("failed to write raster test image");
}

fn write_tar_entries(path: &Path, entries: &[(&str, &str)]) {
    let file = File::create(path).expect("failed to create tar");
    let mut builder = TarBuilder::new(file);
    append_tar_entries(&mut builder, entries);
    builder.finish().expect("failed to finish tar");
}

fn write_tar_gz_entries(path: &Path, entries: &[(&str, &str)]) {
    let file = File::create(path).expect("failed to create tar.gz");
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = TarBuilder::new(encoder);
    append_tar_entries(&mut builder, entries);
    builder.finish().expect("failed to finish tar.gz");
}

fn append_tar_entries<W: Write>(builder: &mut TarBuilder<W>, entries: &[(&str, &str)]) {
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

fn append_tar_directories<W: Write>(builder: &mut TarBuilder<W>, path: &Path) {
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

fn write_xz_compressed_file(path: &Path, contents: &[u8]) -> bool {
    let source = path.with_extension("");
    fs::write(&source, contents).expect("failed to write xz staging file");

    let created = Command::new("xz").arg("-zk").arg(&source).status();
    let _ = fs::remove_file(&source);
    created.as_ref().is_ok_and(|status| status.success()) && path.exists()
}

fn put_u16_le(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u32_be(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn put_u64_le(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn sample_pe_exe_bytes() -> Vec<u8> {
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

fn sample_elf_shared_object_bytes() -> Vec<u8> {
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

fn sample_macho_dylib_bytes() -> Vec<u8> {
    let mut bytes = vec![0u8; 32];
    bytes[0..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
    put_u32_le(&mut bytes, 4, 0x0100000c);
    put_u32_le(&mut bytes, 12, 6);
    put_u32_le(&mut bytes, 16, 12);
    bytes
}

fn sample_dos_mz_bytes() -> Vec<u8> {
    let mut bytes = vec![0u8; 64];
    bytes[0..2].copy_from_slice(b"MZ");
    bytes
}

fn sample_macho_fat_bytes() -> Vec<u8> {
    let mut bytes = vec![0u8; 48];
    bytes[0..4].copy_from_slice(&[0xca, 0xfe, 0xba, 0xbe]);
    put_u32_be(&mut bytes, 4, 2);
    put_u32_be(&mut bytes, 8, 7);
    put_u32_be(&mut bytes, 28, 0x0100000c);
    bytes
}

fn sample_pdf_bytes() -> Vec<u8> {
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

#[test]
fn markdown_preview_formats_headings_and_lists() {
    let root = temp_path("markdown");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(&path, "# Heading\n- item\n`inline`\n").expect("failed to write markdown");

    let preview = build_preview(&file_entry(path.clone()));

    assert_eq!(preview.kind, PreviewKind::Markdown);
    assert_eq!(preview.lines[0].spans[0].content, "Heading");
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content == "inline"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_formats_inline_emphasis_mid_line() {
    let root = temp_path("markdown-inline");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(&path, "hello **bold** world\n").expect("failed to write markdown");

    let preview = build_preview(&file_entry(path.clone()));
    let line = &preview.lines[0];

    assert_eq!(preview.kind, PreviewKind::Markdown);
    assert!(line.spans.iter().any(|span| span.content == "hello "));
    assert!(line.spans.iter().any(|span| span.content == "bold"));
    assert!(line.spans.iter().any(|span| span.content == " world"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_renders_fenced_code_blocks() {
    let root = temp_path("markdown-fence");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(&path, "```rust\nfn main() {}\n```\n").expect("failed to write markdown");

    let preview = build_preview(&file_entry(path.clone()));

    assert_eq!(preview.kind, PreviewKind::Markdown);
    assert_eq!(preview.lines[0].spans[1].content, "rust");
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line_text(line).contains("fn main() {}"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_routes_fence_aliases_through_registry() {
    let root = temp_path("markdown-fence-aliases");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(
        &path,
        "```js\nconst value = 1;\n```\n\n```kitty\nfont_size 11.5\n```\n",
    )
    .expect("failed to write markdown");

    let preview = build_preview(&file_entry(path.clone()));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Markdown);
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content == "js"))
    );
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content == "kitty"))
    );

    let js_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("const value = 1;"))
        .expect("expected highlighted js line");
    assert_ne!(span_color(js_line, "const"), Some(code_palette.fg));

    let kitty_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("font_size 11.5"))
        .expect("expected highlighted kitty line");
    assert_eq!(
        span_color(kitty_line, "font_size"),
        Some(code_palette.function)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_renders_links() {
    let root = temp_path("markdown-links");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(&path, "open [elio](https://example.com)\n").expect("failed to write markdown");

    let preview = build_preview(&file_entry(path));
    let line = &preview.lines[0];

    assert_eq!(preview.kind, PreviewKind::Markdown);
    let link_span = line
        .spans
        .iter()
        .find(|span| span.content == "elio")
        .expect("link label should be rendered");
    assert!(link_span.style.add_modifier.contains(Modifier::UNDERLINED));
    assert!(line_text(line).contains("(https://example.com)"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_adds_spacing_between_blocks() {
    let root = temp_path("markdown-spacing");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(
        &path,
        "# Heading\nParagraph text\n\n```rust\nlet x = 1;\n```\n",
    )
    .expect("failed to write markdown");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Markdown);
    assert!(preview.lines.iter().any(|line| line.spans.is_empty()));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_renders_nested_emphasis() {
    let root = temp_path("markdown-nested");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(&path, "**bold and *italic***\n").expect("failed to write markdown");

    let preview = build_preview(&file_entry(path));
    let line = &preview.lines[0];

    assert_eq!(preview.kind, PreviewKind::Markdown);
    let italic_span = line
        .spans
        .iter()
        .find(|span| span.content == "italic")
        .expect("nested italic content should be rendered");
    assert!(italic_span.style.add_modifier.contains(Modifier::BOLD));
    assert!(italic_span.style.add_modifier.contains(Modifier::ITALIC));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_preview_renders_mixed_lists() {
    let root = temp_path("markdown-mixed-lists");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("README.md");
    fs::write(&path, "1. first\n   - nested\n2. second\n").expect("failed to write markdown");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Markdown);
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content == "1. "))
    );
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content.contains("• ")))
    );
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content == "2. "))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn markdown_license_preview_keeps_detected_detail() {
    let root = temp_path("markdown-license");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("LICENSE.md");
    fs::write(
        &path,
        "# SPDX-License-Identifier: Apache-2.0\n\nLicensed under the Apache License, Version 2.0.\n",
    )
    .expect("failed to write markdown license");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Markdown);
    assert_eq!(preview.detail.as_deref(), Some("Apache License 2.0"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn plain_text_license_preview_shows_specific_license_detail() {
    let root = temp_path("plain-license");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("LICENSE");
    fs::write(
        &path,
        "MIT License\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the \"Software\"), to deal\nin the Software without restriction, including without limitation the rights\nto use, copy, modify, merge, publish, distribute, sublicense, and/or sell\ncopies of the Software, and to permit persons to whom the Software is\nfurnished to do so.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND.\n",
    )
    .expect("failed to write license");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Text);
    assert_eq!(preview.detail.as_deref(), Some("MIT License"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn code_preview_includes_line_numbers() {
    let root = temp_path("code");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.rs");
    fs::write(&path, "fn main() {}\n").expect("failed to write code");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.lines[0].spans[0].content.contains("1"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn c_preview_uses_code_renderer() {
    let root = temp_path("c");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.c");
    fs::write(
        &path,
        "#include <stdio.h>\nint main(void) {\n    printf(\"hello\\n\");\n}\n",
    )
    .expect("failed to write c source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some_and(|detail| detail.contains('C')));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("printf"))
    );
    assert_ne!(span_color(&preview.lines[0], "#"), Some(code_palette.fg));
    assert_ne!(span_color(&preview.lines[1], "int"), Some(code_palette.fg));
    assert_ne!(
        span_color(&preview.lines[2], "printf"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn python_preview_uses_code_renderer_with_colors() {
    let root = temp_path("python");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.py");
    fs::write(
            &path,
            "@decorator\nclass Greeter:\n    async def greet(self, name: str) -> str:\n        \"\"\"Return greeting.\"\"\"\n        return f\"hi {name}\"\n",
        )
        .expect("failed to write python source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .is_some_and(|detail| detail.contains("Python"))
    );
    assert_ne!(
        span_color(&preview.lines[1], "class"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[1], "Greeter"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[2], "async"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[2], "greet"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[4], "return"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[4], "f\"hi {name}\""),
        Some(code_palette.fg)
    );
    assert!(line_text(&preview.lines[3]).contains("Return greeting."));
    assert!(line_text(&preview.lines[4]).contains("f\"hi {name}\""));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn javascript_preview_uses_code_renderer_with_colors() {
    let root = temp_path("javascript");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.js");
    fs::write(
        &path,
        "export class Greeter {\n  greet(name) { return console.log(`hi ${name}`); }\n}\n",
    )
    .expect("failed to write javascript source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .is_some_and(|detail| detail.contains("JavaScript"))
    );
    assert_ne!(
        span_color(&preview.lines[0], "export"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[0], "Greeter"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[1], "return"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn nix_preview_uses_code_renderer() {
    let root = temp_path("nix");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("flake.nix");
    fs::write(
            &path,
            "{ description = \"elio\"; outputs = { self }: { packages.x86_64-linux.default = self; }; }\n",
        )
        .expect("failed to write nix source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some_and(|detail| detail.contains("Nix")));
    assert_eq!(
        span_color(&preview.lines[0], "description"),
        Some(code_palette.parameter)
    );
    assert!(line_has_color(&preview.lines[0], code_palette.string));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("description"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cmake_preview_uses_code_renderer() {
    let root = temp_path("cmake");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("CMakeLists.txt");
    fs::write(
        &path,
        "cmake_minimum_required(VERSION 3.28)\nproject(elio)\nadd_executable(elio main.cpp)\n",
    )
    .expect("failed to write cmake source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .is_some_and(|detail| detail.contains("CMake"))
    );
    assert_ne!(
        span_color(&preview.lines[2], "add_executable"),
        Some(code_palette.fg)
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("add_executable"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn generic_lockfile_uses_code_renderer() {
    let root = temp_path("lock");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("deps.lock");
    fs::write(&path, "[packages]\nelio=1.0.0\n").expect("failed to write lockfile");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Lockfile"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("elio"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn makefile_preview_uses_code_renderer() {
    let root = temp_path("makefile");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("Makefile");
    fs::write(
        &path,
        "CC := clang\n.PHONY: build\nbuild: main.o util.o\n\t$(CC) -o app main.o util.o\n",
    )
    .expect("failed to write makefile");

    let preview = build_preview(&file_entry(path.clone()));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Make"))
    );
    assert!(line_texts.iter().any(|text| text.contains(".PHONY")));
    assert!(line_texts.iter().any(|text| text.contains("$(CC)")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn html_preview_uses_code_renderer() {
    let root = temp_path("html");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("index.html");
    fs::write(
        &path,
        "<!DOCTYPE html>\n<div class=\"app\" data-id=\"42\">elio</div>\n",
    )
    .expect("failed to write html");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some());
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("div"))
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("class"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn css_preview_uses_code_renderer() {
    let root = temp_path("css");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("styles.css");
    fs::write(&path, ".app {\n  color: #fff;\n  margin: 12px;\n}\n").expect("failed to write css");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some());
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("color"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn docx_preview_shows_document_metadata() {
    let root = temp_path("docx");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("report.docx");
    write_zip_entries(
        &path,
        &[
            (
                "docProps/core.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                        xmlns:dc="http://purl.org/dc/elements/1.1/"
                        xmlns:dcterms="http://purl.org/dc/terms/">
                      <dc:title>Quarterly Report</dc:title>
                      <dc:creator>Regueiro</dc:creator>
                      <dcterms:created>2026-03-11T09:00:00Z</dcterms:created>
                    </cp:coreProperties>"#,
            ),
            (
                "docProps/app.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
                      <Application>LibreOffice</Application>
                      <Pages>12</Pages>
                      <Words>4238</Words>
                    </Properties>"#,
            ),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("DOCX document"));
    assert_eq!(line_texts[0], "Document");
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("Format") || !text.contains("DOCX document"))
    );
    assert!(line_texts.iter().any(|text| text == "People"));
    assert!(line_texts.iter().any(|text| text == "Dates"));
    assert!(line_texts.iter().any(|text| text == "Stats"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Quarterly Report"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Regueiro")));
    assert!(line_texts.iter().any(|text| text.contains("4,238")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Created") && text.contains("Mar 11, 2026 09:00 UTC"))
    );
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("2026-03-11T09:00:00Z"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("LibreOffice"))
    );
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("ApplicationLibreOffice"))
    );
    assert!(
        line_texts
            .iter()
            .position(|text| text == "Document")
            .unwrap()
            < line_texts.iter().position(|text| text == "People").unwrap()
    );
    assert!(
        line_texts.iter().position(|text| text == "People").unwrap()
            < line_texts.iter().position(|text| text == "Dates").unwrap()
    );
    assert!(
        line_texts.iter().position(|text| text == "Dates").unwrap()
            < line_texts.iter().position(|text| text == "Stats").unwrap()
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn odt_preview_shows_document_metadata() {
    let root = temp_path("odt");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("report.odt");
    write_zip_entries(
        &path,
        &[(
            "meta.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                    xmlns:dc="http://purl.org/dc/elements/1.1/"
                    xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0">
                  <office:meta>
                    <dc:title>Project Notes</dc:title>
                    <meta:initial-creator>Elio</meta:initial-creator>
                    <meta:creation-date>2026-03-10T18:00:00Z</meta:creation-date>
                    <meta:generator>LibreOffice</meta:generator>
                    <meta:document-statistic meta:page-count="3" meta:word-count="980" meta:character-count="6400"/>
                  </office:meta>
                </office:document-meta>"#,
        )],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("ODT document"));
    assert_eq!(line_texts[0], "Document");
    assert!(line_texts.iter().any(|text| text == "People"));
    assert!(line_texts.iter().any(|text| text == "Dates"));
    assert!(line_texts.iter().any(|text| text == "Stats"));
    assert!(line_texts.iter().any(|text| text.contains("Project Notes")));
    assert!(line_texts.iter().any(|text| text.contains("LibreOffice")));
    assert!(line_texts.iter().any(|text| text.contains("980")));
    assert!(line_texts.iter().any(|text| text.contains("6,400")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Created") && text.contains("Mar 10, 2026 18:00 UTC"))
    );
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("2026-03-10T18:00:00Z"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn pptx_preview_shows_presentation_metadata() {
    let root = temp_path("pptx");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("deck.pptx");
    write_zip_entries(
        &path,
        &[
            (
                "docProps/core.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                        xmlns:dc="http://purl.org/dc/elements/1.1/"
                        xmlns:dcterms="http://purl.org/dc/terms/">
                      <dc:title>Launch Deck</dc:title>
                      <dc:creator>Elio</dc:creator>
                      <dcterms:modified>2026-03-12T09:30:00Z</dcterms:modified>
                    </cp:coreProperties>"#,
            ),
            (
                "docProps/app.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
                      <Application>PowerPoint</Application>
                      <Slides>24</Slides>
                      <Notes>6</Notes>
                      <HiddenSlides>2</HiddenSlides>
                    </Properties>"#,
            ),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("PPTX presentation"));
    assert!(line_texts.iter().any(|text| text.contains("Launch Deck")));
    assert!(line_texts.iter().any(|text| text.contains("PowerPoint")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Slides") && text.contains("24"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Notes") && text.contains("6"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Hidden Slides") && text.contains("2"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn xlsx_preview_shows_spreadsheet_metadata() {
    let root = temp_path("xlsx");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("budget.xlsx");
    write_zip_entries(
        &path,
        &[
            (
                "docProps/core.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                        xmlns:dc="http://purl.org/dc/elements/1.1/">
                      <dc:title>Q2 Budget</dc:title>
                      <dc:creator>Finance Team</dc:creator>
                    </cp:coreProperties>"#,
            ),
            (
                "docProps/app.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
                      <Application>Excel</Application>
                      <Company>Elio Labs</Company>
                      <Manager>Regueiro</Manager>
                    </Properties>"#,
            ),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("XLSX spreadsheet"));
    assert!(line_texts.iter().any(|text| text.contains("Q2 Budget")));
    assert!(line_texts.iter().any(|text| text.contains("Finance Team")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("Excel"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Company") && text.contains("Elio Labs"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Manager") && text.contains("Regueiro"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn ods_preview_shows_spreadsheet_statistics() {
    let root = temp_path("ods");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("budget.ods");
    write_zip_entries(
        &path,
        &[(
            "meta.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                    xmlns:dc="http://purl.org/dc/elements/1.1/"
                    xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0">
                  <office:meta>
                    <dc:title>Operations Budget</dc:title>
                    <meta:initial-creator>Elio</meta:initial-creator>
                    <meta:generator>LibreOffice Calc</meta:generator>
                    <meta:document-statistic meta:table-count="4" meta:cell-count="512" meta:object-count="2"/>
                  </office:meta>
                </office:document-meta>"#,
        )],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("ODS spreadsheet"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Operations Budget"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("LibreOffice Calc"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Tables") && text.contains("4"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Cells") && text.contains("512"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Objects") && text.contains("2"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_preview_shows_package_metadata() {
    let root = temp_path("epub");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("novel.epub");
    write_zip_entries(
        &path,
        &[
            (
                "META-INF/container.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
                      <rootfiles>
                        <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
                      </rootfiles>
                    </container>"#,
            ),
            (
                "OPS/package.opf",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
                      <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                        <dc:title>Elio Handbook</dc:title>
                        <dc:creator>Regueiro</dc:creator>
                        <dc:language>en</dc:language>
                        <dc:publisher>Elio Docs</dc:publisher>
                        <dc:identifier>urn:uuid:elio-handbook</dc:identifier>
                        <dc:date>2026-03-12T08:00:00Z</dc:date>
                      </metadata>
                    </package>"#,
            ),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("Elio Handbook"));
    assert_eq!(
        preview.status_note.as_deref(),
        Some("EPUB ebook  •  Regueiro")
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Variant") && text.contains("EPUB package"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Elio Handbook")));
    assert!(line_texts.iter().any(|text| text.contains("Regueiro")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Language") && text.contains("en"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Publisher") && text.contains("Elio Docs"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Identifier") && text.contains("urn:uuid:elio-handbook"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_preview_shows_contents_and_excerpt() {
    let root = temp_path("epub-excerpt");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("story.epub");
    write_zip_entries(
        &path,
        &[
            (
                "META-INF/container.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
                      <rootfiles>
                        <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
                      </rootfiles>
                    </container>"#,
            ),
            (
                "OPS/package.opf",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
                      <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                        <dc:title>Elio Story</dc:title>
                        <dc:creator>Regueiro</dc:creator>
                      </metadata>
                      <manifest>
                        <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                        <item id="chapter-1" href="text/chapter-1.xhtml" media-type="application/xhtml+xml"/>
                        <item id="chapter-2" href="text/chapter-2.xhtml" media-type="application/xhtml+xml"/>
                      </manifest>
                      <spine>
                        <itemref idref="chapter-1"/>
                        <itemref idref="chapter-2"/>
                      </spine>
                    </package>"#,
            ),
            (
                "OPS/nav.xhtml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
                      <body>
                        <nav epub:type="toc">
                          <ol>
                            <li><a href="text/chapter-1.xhtml">Opening</a></li>
                            <li><a href="text/chapter-2.xhtml">Second Step</a></li>
                          </ol>
                        </nav>
                      </body>
                    </html>"#,
            ),
            (
                "OPS/text/chapter-1.xhtml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <html xmlns="http://www.w3.org/1999/xhtml">
                      <body>
                        <h1>Opening</h1>
                        <p>Elio begins with a small terminal window and a very opinionated file browser.</p>
                      </body>
                    </html>"#,
            ),
            (
                "OPS/text/chapter-2.xhtml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <html xmlns="http://www.w3.org/1999/xhtml">
                      <body>
                        <h2>Second Step</h2>
                        <p>The preview pane grows into an actual reading surface instead of stopping at metadata.</p>
                      </body>
                    </html>"#,
            ),
        ],
    );

    let preview = build_preview(&file_entry(path.clone()));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("EPUB ebook"));
    assert_eq!(preview.status_note.as_deref(), None);
    assert_eq!(preview.ebook_section_index, Some(0));
    assert_eq!(preview.ebook_section_count, Some(2));
    assert_eq!(preview.ebook_section_title.as_deref(), Some("Opening"));
    assert!(line_texts.iter().any(|text| {
        text.contains(
            "Elio begins with a small terminal window and a very opinionated file browser.",
        )
    }));

    let second_preview = build_preview_with_options(
        &file_entry(path.clone()),
        &PreviewRequestOptions::EpubSection(1),
    );
    let second_line_texts: Vec<_> = second_preview.lines.iter().map(line_text).collect();
    assert_eq!(second_preview.ebook_section_index, Some(1));
    assert_eq!(second_preview.ebook_section_count, Some(2));
    assert_eq!(
        second_preview.ebook_section_title.as_deref(),
        Some("Second Step")
    );
    assert!(second_line_texts.iter().any(|text| {
        text.contains(
            "The preview pane grows into an actual reading surface instead of stopping at metadata."
        )
    }));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_preview_extracts_cover_image() {
    let root = temp_path("epub-cover");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("cover.epub");
    let source_cover = root.join("source-cover.png");
    write_test_raster_image(&source_cover, ImageFormat::Png, 160, 240);
    let cover_bytes = fs::read(&source_cover).expect("failed to read cover image");

    let file = File::create(&path).expect("failed to create epub");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, contents) in [
        (
            "META-INF/container.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
                  <rootfiles>
                    <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
                  </rootfiles>
                </container>"#,
        ),
        (
            "OPS/package.opf",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
                  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                    <dc:title>Covered Story</dc:title>
                    <meta name="cover" content="cover-image"/>
                  </metadata>
                  <manifest>
                    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                    <item id="cover-image" href="images/cover.png" media-type="image/png"/>
                    <item id="chapter-1" href="text/chapter-1.xhtml" media-type="application/xhtml+xml"/>
                  </manifest>
                  <spine>
                    <itemref idref="chapter-1"/>
                  </spine>
                </package>"#,
        ),
        (
            "OPS/nav.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
                  <body>
                    <nav epub:type="toc">
                      <ol>
                        <li><a href="text/chapter-1.xhtml">Opening</a></li>
                      </ol>
                    </nav>
                  </body>
                </html>"#,
        ),
        (
            "OPS/text/chapter-1.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml">
                  <body>
                    <p>The cover should be extracted for inline preview.</p>
                  </body>
                </html>"#,
        ),
    ] {
        zip.start_file(name, options)
            .expect("failed to start epub text entry");
        zip.write_all(contents.as_bytes())
            .expect("failed to write epub text entry");
    }
    zip.start_file("OPS/images/cover.png", options)
        .expect("failed to start cover entry");
    zip.write_all(&cover_bytes)
        .expect("failed to write cover entry");
    zip.finish().expect("failed to finish epub");

    let preview = build_preview(&file_entry(path));
    let visual = preview
        .preview_visual
        .clone()
        .expect("cover visual should be extracted");

    assert_eq!(visual.kind, PreviewVisualKind::Cover);
    assert_eq!(visual.layout, PreviewVisualLayout::Inline);
    assert!(visual.path.exists());
    assert!(visual.size > 0);
    assert_eq!(preview.ebook_section_index, Some(0));
    assert_eq!(preview.ebook_section_count, Some(1));

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_preview_uses_section_image_for_fixed_layout_pages() {
    let root = temp_path("epub-fixed-layout");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("fixed-layout.epub");
    let source_cover = root.join("fixed-layout-cover.jpg");
    write_test_raster_image(&source_cover, ImageFormat::Jpeg, 160, 240);
    let cover_bytes = fs::read(&source_cover).expect("failed to read cover image");

    let file = File::create(&path).expect("failed to create epub");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, contents) in [
        (
            "META-INF/container.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
                  <rootfiles>
                    <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
                  </rootfiles>
                </container>"#,
        ),
        (
            "OPS/package.opf",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
                  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                    <dc:title>Fixed Layout Story</dc:title>
                  </metadata>
                  <manifest>
                    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                    <item id="cover" href="images/cover.jpg" media-type="image/jpeg" properties="cover-image"/>
                    <item id="page-1" href="xhtml/page-1.xhtml" media-type="application/xhtml+xml" properties="svg"/>
                  </manifest>
                  <spine>
                    <itemref idref="page-1"/>
                  </spine>
                </package>"#,
        ),
        (
            "OPS/nav.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
                  <body>
                    <nav epub:type="toc">
                      <ol>
                        <li><a href="xhtml/page-1.xhtml">Page 1</a></li>
                      </ol>
                    </nav>
                  </body>
                </html>"#,
        ),
        (
            "OPS/xhtml/page-1.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml">
                  <body>
                    <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
                      <image width="160" height="240" xlink:href="../images/cover.jpg"/>
                    </svg>
                  </body>
                </html>"#,
        ),
    ] {
        zip.start_file(name, options)
            .expect("failed to start epub text entry");
        zip.write_all(contents.as_bytes())
            .expect("failed to write epub text entry");
    }
    zip.start_file("OPS/images/cover.jpg", options)
        .expect("failed to start image entry");
    zip.write_all(&cover_bytes)
        .expect("failed to write image entry");
    zip.finish().expect("failed to finish epub");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let visual = preview
        .preview_visual
        .clone()
        .expect("fixed-layout page image should be extracted");

    assert_eq!(preview.detail.as_deref(), Some("EPUB ebook"));
    assert_eq!(preview.ebook_section_index, Some(0));
    assert_eq!(preview.ebook_section_count, Some(1));
    assert_eq!(preview.ebook_section_title.as_deref(), Some("Page 1"));
    assert_eq!(visual.kind, PreviewVisualKind::PageImage);
    assert_eq!(visual.layout, PreviewVisualLayout::FullHeight);
    assert!(line_texts.is_empty());
    assert!(visual.path.exists());
    assert!(
        visual
            .path
            .parent()
            .is_some_and(|parent| parent.ends_with("elio-epub-asset-v2"))
    );

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn concurrent_fixed_layout_epub_section_builds_keep_shared_image_cache_readable() {
    let root = temp_path("epub-fixed-layout-concurrent");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("fixed-layout.epub");
    let source_image = root.join("shared.jpg");
    write_test_raster_image(&source_image, ImageFormat::Jpeg, 160, 240);
    let image_bytes = fs::read(&source_image).expect("failed to read shared image");

    let file = File::create(&path).expect("failed to create epub");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, contents) in [
        (
            "META-INF/container.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
                  <rootfiles>
                    <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
                  </rootfiles>
                </container>"#,
        ),
        (
            "OPS/package.opf",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
                  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                    <dc:title>Shared Fixed Layout</dc:title>
                  </metadata>
                  <manifest>
                    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                    <item id="page-1" href="xhtml/page-1.xhtml" media-type="application/xhtml+xml" properties="svg"/>
                    <item id="page-2" href="xhtml/page-2.xhtml" media-type="application/xhtml+xml" properties="svg"/>
                  </manifest>
                  <spine>
                    <itemref idref="page-1"/>
                    <itemref idref="page-2"/>
                  </spine>
                </package>"#,
        ),
        (
            "OPS/nav.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
                  <body>
                    <nav epub:type="toc">
                      <ol>
                        <li><a href="xhtml/page-1.xhtml">Page 1</a></li>
                        <li><a href="xhtml/page-2.xhtml">Page 2</a></li>
                      </ol>
                    </nav>
                  </body>
                </html>"#,
        ),
        (
            "OPS/xhtml/page-1.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml">
                  <body>
                    <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
                      <image width="160" height="240" xlink:href="../images/shared.jpg"/>
                    </svg>
                  </body>
                </html>"#,
        ),
        (
            "OPS/xhtml/page-2.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml">
                  <body>
                    <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
                      <image width="160" height="240" xlink:href="../images/shared.jpg"/>
                    </svg>
                  </body>
                </html>"#,
        ),
    ] {
        zip.start_file(name, options)
            .expect("failed to start epub entry");
        zip.write_all(contents.as_bytes())
            .expect("failed to write epub entry");
    }
    zip.start_file("OPS/images/shared.jpg", options)
        .expect("failed to start shared image entry");
    zip.write_all(&image_bytes)
        .expect("failed to write shared image entry");
    zip.finish().expect("failed to finish epub");

    let path = Arc::new(path);
    let barrier = Arc::new(Barrier::new(9));
    let mut handles = Vec::new();
    for worker in 0..8 {
        let path = Arc::clone(&path);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            for iteration in 0..20 {
                let preview = build_preview_with_options(
                    &file_entry((*path).clone()),
                    &PreviewRequestOptions::EpubSection((worker + iteration) % 2),
                );
                let visual = preview
                    .preview_visual
                    .as_ref()
                    .expect("fixed-layout section should expose a page image");
                let dimensions = image::ImageReader::open(&visual.path)
                    .expect("cached shared image should open")
                    .with_guessed_format()
                    .expect("shared image format should be detected")
                    .into_dimensions()
                    .expect("shared image dimensions should be readable");
                assert_eq!(dimensions, (160, 240));
            }
        }));
    }

    barrier.wait();
    for handle in handles {
        handle
            .join()
            .expect("concurrent fixed-layout worker should finish");
    }

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_package_cache_reuses_parse_across_section_switches() {
    super::document::clear_epub_package_cache();

    let root = temp_path("epub-package-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("story.epub");
    super::document::reset_epub_package_parse_count(&path);
    write_zip_entries(
        &path,
        &[
            (
                "META-INF/container.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
                      <rootfiles>
                        <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
                      </rootfiles>
                    </container>"#,
            ),
            (
                "OPS/package.opf",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
                      <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                        <dc:title>Cached Story</dc:title>
                      </metadata>
                      <manifest>
                        <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                        <item id="chapter-1" href="text/chapter-1.xhtml" media-type="application/xhtml+xml"/>
                        <item id="chapter-2" href="text/chapter-2.xhtml" media-type="application/xhtml+xml"/>
                      </manifest>
                      <spine>
                        <itemref idref="chapter-1"/>
                        <itemref idref="chapter-2"/>
                      </spine>
                    </package>"#,
            ),
            (
                "OPS/nav.xhtml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
                      <body>
                        <nav epub:type="toc">
                          <ol>
                            <li><a href="text/chapter-1.xhtml">Opening</a></li>
                            <li><a href="text/chapter-2.xhtml">Second Step</a></li>
                          </ol>
                        </nav>
                      </body>
                    </html>"#,
            ),
            (
                "OPS/text/chapter-1.xhtml",
                r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>One.</p></body></html>"#,
            ),
            (
                "OPS/text/chapter-2.xhtml",
                r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>Two.</p></body></html>"#,
            ),
        ],
    );

    let _ = build_preview(&file_entry(path.clone()));
    let _ = build_preview_with_options(
        &file_entry(path.clone()),
        &PreviewRequestOptions::EpubSection(1),
    );

    assert_eq!(super::document::epub_package_parse_count(&path), 1);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn pdf_preview_shows_pdfinfo_metadata() {
    if Command::new("pdfinfo").arg("-v").output().is_err() {
        return;
    }

    let root = temp_path("pdf");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("report.pdf");
    fs::write(&path, sample_pdf_bytes()).expect("failed to write pdf fixture");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("PDF document"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Variant") && text.contains("PDF 1.4"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Quarterly Report"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Regueiro")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("Elio"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Pages") && text.contains("1"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Producer") && text.contains("Elio Test Suite"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn xml_preview_uses_code_renderer() {
    let root = temp_path("xml");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("layout.xml");
    fs::write(&path, "<?xml version=\"1.0\"?>\n<layout id=\"main\" />\n")
        .expect("failed to write xml");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some());
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("layout"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn toml_preview_uses_structured_renderer() {
    let root = temp_path("toml");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("config.toml");
    fs::write(
        &path,
        "[package]\nname = \"elio\"\nversion = \"0.1.0\"\n\n[server]\nport = 3000\n",
    )
    .expect("failed to write toml");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("TOML"));
    let lines = preview
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    assert!(lines.iter().any(|line| line.contains("[package]")));
    assert!(lines.iter().any(|line| line.contains("name = \"elio\"")));
    assert!(lines.iter().any(|line| line.contains("[server]")));
    assert!(lines.iter().any(|line| line.contains("port = 3000")));
    assert!(!lines.iter().any(|line| line.contains("root: object")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn desktop_preview_uses_code_renderer() {
    let root = temp_path("desktop-entry");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("app.desktop");
    fs::write(
        &path,
        "[Desktop Entry]\nName=エリオ\nName[ja]=エリオ\nExec=elio\nTerminal=false\n",
    )
    .expect("failed to write desktop entry");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail == "Desktop Entry")
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("エリオ"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn directive_conf_preview_is_used_for_ambiguous_conf() {
    let root = temp_path("directive-conf-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("custom.conf");
    fs::write(
        &path,
        "font_size 11.5\nforeground #c0c6e2\nmap ctrl+c copy_to_clipboard\n",
    )
    .expect("failed to write directive conf");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Directive config"));
    assert_eq!(
        span_color(&preview.lines[0], "font_size"),
        Some(code_palette.function)
    );
    assert_eq!(
        span_color(&preview.lines[1], "#c0c6e2"),
        Some(code_palette.constant)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn ini_style_conf_preview_uses_ini_highlighting() {
    let root = temp_path("ini-conf-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("settings.conf");
    fs::write(&path, "[Settings]\ncolor=blue\nenabled=true\n").expect("failed to write ini conf");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("INI"));
    assert_eq!(
        span_color(&preview.lines[0], "[Settings]"),
        Some(code_palette.r#type)
    );
    assert_eq!(
        span_color(&preview.lines[1], "color"),
        Some(code_palette.parameter)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn shell_style_conf_preview_uses_shell_highlighting() {
    let root = temp_path("shell-conf-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("module.conf");
    fs::write(
        &path,
        "MAKE=\"make -C src/ KERNELDIR=/lib/modules/${kernelver}/build\"\nAUTOINSTALL=yes\n",
    )
    .expect("failed to write shell conf");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Shell"));
    assert_ne!(span_color(&preview.lines[0], "MAKE"), Some(code_palette.fg));
    assert!(line_text(&preview.lines[0]).contains("${kernelver}"));
    assert_ne!(
        span_color(
            &preview.lines[0],
            "\"make -C src/ KERNELDIR=/lib/modules/${kernelver}/build\""
        ),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn config_modeline_can_force_directive_preview() {
    let root = temp_path("kitty-conf-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("settings.conf");
    fs::write(&path, "# vim:ft=kitty\n[Settings]\nforeground #c0c6e2\n")
        .expect("failed to write modeline conf");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Kitty"));
    assert_eq!(
        span_color(&preview.lines[2], "foreground"),
        Some(code_palette.function)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn pkgbuild_preview_uses_shell_renderer() {
    let root = temp_path("pkgbuild");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("PKGBUILD");
    fs::write(
        &path,
        "pkgname=elio\nbuild() {\n  cargo build --release\n}\n",
    )
    .expect("failed to write pkgbuild");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn shell_script_preview_uses_code_renderer() {
    let root = temp_path("shell");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("deploy.sh");
    fs::write(
            &path,
            "#!/usr/bin/env bash\nNAME=elio\nif [ -n \"$NAME\" ]; then\n  printf '%s\\n' \"$(whoami)\"\nfi\n",
        )
        .expect("failed to write shell script");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Shell"))
    );
    assert!(line_texts.iter().any(|text| text.contains("printf")));
    assert!(line_texts.iter().any(|text| text.contains("$(whoami)")));
    assert_ne!(span_color(&preview.lines[2], "if"), Some(code_palette.fg));
    assert_ne!(
        span_color(&preview.lines[3], "printf"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[3], "$(whoami)"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn shell_dotfile_preview_uses_code_renderer() {
    let root = temp_path("shell-dotfile");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join(".bashrc");
    fs::write(
        &path,
        "export PATH=\"$HOME/bin:$PATH\"\nalias ll='ls -la'\n",
    )
    .expect("failed to write shell dotfile");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Bash"))
    );
    assert!(line_texts.iter().any(|text| text.contains("export")));
    assert!(line_texts.iter().any(|text| text.contains("alias")));
    assert_ne!(
        span_color(&preview.lines[0], "export"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[1], "alias"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn zsh_preview_uses_shell_specific_support() {
    let root = temp_path("zsh");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("prompt.zsh");
    fs::write(
        &path,
        "autoload -U colors && colors\nprompt_elio() {\n  print -P '%F{blue}%~%f'\n}\n",
    )
    .expect("failed to write zsh script");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some());
    assert!(line_texts.iter().any(|text| text.contains("autoload")));
    assert!(line_texts.iter().any(|text| text.contains("prompt_elio")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn keys_preview_uses_highlighting_renderer() {
    let root = temp_path("keys");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("bindings.keys");
    fs::write(&path, "ctrl+h=left\nctrl+l=right\n").expect("failed to write keys");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Keys file"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn log_preview_uses_structured_renderer() {
    let root = temp_path("log");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("server.log");
    fs::write(
        &path,
        "2026-03-10T12:00:00Z ERROR request_id=42 path=/login failed\n",
    )
    .expect("failed to write log");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Log"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("ERROR"))
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("request_id"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn multiline_log_preview_keeps_stack_trace_context() {
    let root = temp_path("log-multiline");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("server.log");
    fs::write(
        &path,
        "2026-03-10T12:00:00Z ERROR request_id=42 msg=\"request failed\"\n\
             \tat service.handle (/srv/app.js:10)\n\
             Caused by: timeout\n\
             2026-03-10T12:00:01Z INFO request_id=42 recovered\n",
    )
    .expect("failed to write log");

    let preview = build_preview(&file_entry(path));
    let rendered = preview
        .lines
        .iter()
        .map(Line::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Log"));
    assert!(rendered.contains("request failed"));
    assert!(rendered.contains("Caused by: timeout"));
    assert!(rendered.contains("recovered"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn unstructured_log_preview_uses_log_highlighting() {
    let root = temp_path("log-highlighting");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("notes.log");
    fs::write(
        &path,
        "starting application\nloading configuration\nready\n",
    )
    .expect("failed to write log");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Log file"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("starting application"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn torrent_preview_shows_single_file_metadata_and_trackers() {
    let root = temp_path("torrent");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("sample.torrent");
    let bytes = bencode_dict(vec![
        ("announce", bencode_str("https://tracker.test")),
        (
            "announce-list",
            bencode_list(vec![bencode_list(vec![
                bencode_str("https://tracker.test"),
                bencode_str("https://backup.test"),
            ])]),
        ),
        ("comment", bencode_str("test torrent")),
        ("created by", bencode_str("elio")),
        (
            "info",
            bencode_dict(vec![
                ("length", bencode_int(12_345)),
                ("name", bencode_str("file.txt")),
                ("piece length", bencode_int(262_144)),
                ("pieces", bencode_bytes(b"12345678901234567890")),
                ("private", bencode_int(1)),
            ]),
        ),
    ]);
    fs::write(&path, bytes).expect("failed to write torrent");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Text);
    assert_eq!(preview.detail.as_deref(), Some("BitTorrent file"));
    assert_eq!(line_texts.first().map(String::as_str), Some("Torrent"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Name") && text.contains("file.txt"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Mode") && text.contains("Single-file"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Private")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Trackers") && text.contains("2 across 1 tier"))
    );
    assert!(line_texts.iter().any(|text| text == "Trackers"));
    assert!(line_texts.iter().any(|text| {
        text.contains("Tier 1") && text.contains("tracker.test") && text.contains("backup.test")
    }));
    assert!(line_texts.iter().any(|text| text == "Contents"));
    assert!(line_texts.iter().any(|text| text.contains("file.txt")));
    assert!(!preview.truncated);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn torrent_preview_shows_multifile_contents_tree() {
    let root = temp_path("torrent-multifile");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("series.torrent");
    let bytes = bencode_dict(vec![
        (
            "announce-list",
            bencode_list(vec![
                bencode_list(vec![
                    bencode_str("https://tracker.one"),
                    bencode_str("https://tracker.two"),
                ]),
                bencode_list(vec![bencode_str("https://backup.tld/announce")]),
            ]),
        ),
        ("created by", bencode_str("elio")),
        (
            "info",
            bencode_dict(vec![
                (
                    "files",
                    bencode_list(vec![
                        bencode_dict(vec![
                            ("length", bencode_int(100)),
                            (
                                "path",
                                bencode_list(vec![
                                    bencode_str("season-01"),
                                    bencode_str("ep1.mkv"),
                                ]),
                            ),
                        ]),
                        bencode_dict(vec![
                            ("length", bencode_int(200)),
                            (
                                "path.utf-8",
                                bencode_list(vec![
                                    bencode_str("season-01"),
                                    bencode_str("ep2.mkv"),
                                ]),
                            ),
                        ]),
                    ]),
                ),
                ("name", bencode_str("series")),
                ("piece length", bencode_int(65_536)),
                (
                    "pieces",
                    bencode_bytes(b"1234567890123456789012345678901234567890"),
                ),
                ("private", bencode_int(0)),
            ]),
        ),
    ]);
    fs::write(&path, bytes).expect("failed to write torrent");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Text);
    assert_eq!(preview.detail.as_deref(), Some("BitTorrent file"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Mode") && text.contains("Multi-file"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Files") && text.contains("2"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Trackers") && text.contains("3 across 2 tiers"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Tier 2")));
    assert!(line_texts.iter().any(|text| text.contains("series/")));
    assert!(line_texts.iter().any(|text| text.contains("season-01/")));
    assert!(line_texts.iter().any(|text| text.contains("ep1.mkv")));
    assert!(line_texts.iter().any(|text| text.contains("ep2.mkv")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Privacy") && text.contains("Public"))
    );
    assert!(!preview.truncated);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn srt_preview_keeps_specific_type_detail() {
    let root = temp_path("srt");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("movie.srt");
    fs::write(&path, "1\n00:00:01,000 --> 00:00:02,000\nHello\n").expect("failed to write srt");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Text);
    assert_eq!(preview.detail.as_deref(), Some("SubRip subtitles"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn iso_binary_preview_keeps_specific_type_detail() {
    let root = temp_path("iso");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("disk.iso");
    fs::write(&path, [0x00, 0x81, 0xFE, 0xFF]).expect("failed to write iso");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Binary);
    assert_eq!(preview.detail.as_deref(), Some("ISO disk image"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn iso_metadata_parser_reads_primary_volume_descriptor() {
    let metadata = container::parse_iso_metadata(&sample_iso_descriptors())
        .expect("sample descriptors should parse");

    assert_eq!(metadata.system_id.as_deref(), Some("ELIO_SYS"));
    assert_eq!(metadata.volume_id.as_deref(), Some("ELIO_INSTALL"));
    assert_eq!(metadata.publisher_id.as_deref(), Some("Elio Publisher"));
    assert_eq!(metadata.preparer_id.as_deref(), Some("Elio Builder"));
    assert_eq!(metadata.application_id.as_deref(), Some("Elio Image Tool"));
    assert_eq!(metadata.created_at.as_deref(), Some("2026-03-11 09:00:00"));
    assert_eq!(metadata.modified_at.as_deref(), Some("2026-03-11 10:15:00"));
    assert_eq!(
        metadata.effective_at.as_deref(),
        Some("2026-03-12 00:00:00")
    );
    assert_eq!(metadata.total_size, Some(640 * ISO_SECTOR_SIZE as u64));
    assert!(metadata.bootable);
}

#[test]
fn iso_entry_normalization_reconstructs_parents_and_strips_versions() {
    let entries = container::normalize_archive_entries(
        ["/docs/readme.txt;1", "./EFI/BOOT/", "boot.catalog;1"],
        true,
    );

    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "docs" && entry.is_dir)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "docs/readme.txt" && !entry.is_dir)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "EFI" && entry.is_dir)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "EFI/BOOT" && entry.is_dir)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "boot.catalog" && !entry.is_dir)
    );
}

#[test]
fn iso_preview_renders_metadata_and_tree() {
    let preview = container::render_iso_preview(
        IsoMetadata {
            volume_id: Some("ELIO_INSTALL".to_string()),
            system_id: Some("ELIO_SYS".to_string()),
            total_size: Some(640 * ISO_SECTOR_SIZE as u64),
            bootable: true,
            created_at: Some("2026-03-11 09:00:00".to_string()),
            ..IsoMetadata::default()
        },
        container::normalize_archive_entries(
            ["boot/", "boot/grub/", "boot/grub/grub.cfg", "README.txt"],
            true,
        ),
    );
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let header = preview
        .header_detail(0, 20)
        .expect("iso preview should expose header detail");

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("ISO disk image"));
    assert!(header.contains("ISO disk image"));
    assert_eq!(line_texts.first().map(String::as_str), Some("Image"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Volume") && text.contains("ELIO_INSTALL"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text == "Contents" || text.ends_with("Contents"))
    );
    assert!(line_texts.iter().any(|text| text.contains("boot/")));
    assert!(line_texts.iter().any(|text| text.contains("grub.cfg")));
    assert!(line_texts.iter().any(|text| text.contains("README.txt")));
}

#[test]
fn iso_preview_reports_tree_truncation() {
    let items = (0..320)
        .map(|index| format!("dir/file-{index:03}.txt"))
        .collect::<Vec<_>>();
    let preview = container::render_iso_preview(
        IsoMetadata {
            volume_id: Some("BIG_IMAGE".to_string()),
            ..IsoMetadata::default()
        },
        container::normalize_archive_entries(items.iter().map(String::as_str), true),
    );
    let header = preview
        .header_detail(0, 20)
        .expect("iso preview header should include truncation");

    assert!(preview.truncated);
    assert!(header.contains("showing first"));
}

#[test]
fn iso_preview_lists_contents_when_bsdtar_can_read_image() {
    let root = temp_path("iso-listing");
    let image_root = root.join("image-root");
    fs::create_dir_all(image_root.join("docs")).expect("failed to create image tree");
    fs::write(image_root.join("docs/readme.txt"), "hello").expect("failed to write image file");
    let path = root.join("sample.iso");

    let created = Command::new("bsdtar")
        .arg("-cf")
        .arg(&path)
        .arg("-C")
        .arg(&image_root)
        .arg(".")
        .status();
    if !created.as_ref().is_ok_and(|status| status.success()) {
        fs::remove_dir_all(root).expect("failed to remove temp root");
        return;
    }

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("ISO disk image"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("docs/"))
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("readme.txt"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn zip_preview_renders_archive_summary_and_tree() {
    let root = temp_path("zip-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("bundle.zip");
    write_zip_entries(
        &path,
        &[
            ("docs/readme.txt", "hello"),
            ("src/main.rs", "fn main() {}\n"),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let header = preview
        .header_detail(0, 20)
        .expect("zip preview should expose header detail");

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("ZIP archive"));
    assert!(header.contains("ZIP archive"));
    assert!(line_texts.iter().any(|text| text.trim() == "Summary"));
    assert!(!line_texts.iter().any(|text| text.trim() == "Archive"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Entries") && text.contains("4 total"))
    );
    assert!(line_texts.iter().any(|text| text.contains("docs/")));
    assert!(line_texts.iter().any(|text| text.contains("src/")));
    assert!(line_texts.iter().any(|text| text.contains("readme.txt")));
    assert!(line_texts.iter().any(|text| text.contains("main.rs")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_zip_preview_renders_first_page_without_summary() {
    let root = temp_path("comic-zip-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("issue.cbz");
    let source_cover = root.join("cover.jpg");
    write_test_raster_image(&source_cover, ImageFormat::Jpeg, 160, 240);
    let cover_bytes = fs::read(&source_cover).expect("failed to read cover image");

    let file = File::create(&path).expect("failed to create comic zip");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("001-cover.jpg", options)
        .expect("failed to start cover entry");
    zip.write_all(&cover_bytes)
        .expect("failed to write cover entry");
    zip.start_file("002-page.jpg", options)
        .expect("failed to start page entry");
    zip.write_all(&cover_bytes)
        .expect("failed to write page entry");
    zip.start_file("notes/readme.txt", options)
        .expect("failed to start text entry");
    zip.write_all(b"hello").expect("failed to write text entry");
    zip.finish().expect("failed to finish comic zip");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let visual = preview
        .preview_visual
        .clone()
        .expect("comic zip should expose a page visual");

    assert_eq!(preview.kind, PreviewKind::Comic);
    assert_eq!(preview.detail.as_deref(), Some("Comic ZIP archive"));
    assert_eq!(visual.kind, PreviewVisualKind::PageImage);
    assert_eq!(visual.layout, PreviewVisualLayout::FullHeight);
    let position = preview
        .navigation_position
        .as_ref()
        .expect("comic zip should expose page navigation");
    assert_eq!(position.label, "Page");
    assert_eq!(position.index, 0);
    assert_eq!(position.count, 2);
    assert!(visual.path.exists());
    assert!(line_texts.is_empty());
    assert!(
        !line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("ZIP"))
    );
    assert!(!line_texts.iter().any(|text| text.contains("Packed")));
    assert!(!line_texts.iter().any(|text| text.contains("Archive Size")));
    assert!(!line_texts.iter().any(|text| text.trim() == "Contents"));
    assert!(!line_texts.iter().any(|text| text.contains("001-cover.jpg")));
    assert!(!line_texts.iter().any(|text| text.contains("002-page.jpg")));
    assert!(!line_texts.iter().any(|text| text.contains("notes/")));

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_zip_preview_uses_natural_page_order_and_page_selection() {
    let root = temp_path("comic-zip-pages");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("issue.cbz");
    write_zip_binary_entries(
        &path,
        &[
            ("10.jpg", b"page-ten"),
            ("2.jpg", b"page-two"),
            ("1.jpg", b"page-one"),
        ],
    );

    let first_preview = build_preview(&file_entry(path.clone()));
    let first_visual = first_preview
        .preview_visual
        .as_ref()
        .expect("first page should be extracted");
    let second_preview = build_preview_with_options(
        &file_entry(path.clone()),
        &PreviewRequestOptions::ComicPage(1),
    );
    let second_visual = second_preview
        .preview_visual
        .as_ref()
        .expect("second page should be extracted");
    let third_preview = build_preview_with_options(
        &file_entry(path.clone()),
        &PreviewRequestOptions::ComicPage(2),
    );
    let third_visual = third_preview
        .preview_visual
        .as_ref()
        .expect("third page should be extracted");

    assert_eq!(
        fs::read(&first_visual.path).expect("failed to read first page"),
        b"page-one"
    );
    assert_eq!(
        fs::read(&second_visual.path).expect("failed to read second page"),
        b"page-two"
    );
    assert_eq!(
        fs::read(&third_visual.path).expect("failed to read third page"),
        b"page-ten"
    );
    assert_eq!(
        second_preview
            .navigation_position
            .as_ref()
            .map(|position| position.index),
        Some(1)
    );
    assert_eq!(
        third_preview
            .navigation_position
            .as_ref()
            .map(|position| position.count),
        Some(3)
    );

    let _ = fs::remove_file(first_visual.path.clone());
    let _ = fs::remove_file(second_visual.path.clone());
    let _ = fs::remove_file(third_visual.path.clone());
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_rar_preview_renders_first_page_without_summary() {
    let root = temp_path("comic-rar-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("issue.cbr");
    let source_cover = root.join("cover.jpg");
    write_test_raster_image(&source_cover, ImageFormat::Jpeg, 160, 240);
    let cover_bytes = fs::read(&source_cover).expect("failed to read cover image");
    write_zip_binary_entries(
        &path,
        &[
            ("001-cover.jpg", &cover_bytes),
            ("002-page.jpg", &cover_bytes),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let visual = preview
        .preview_visual
        .clone()
        .expect("comic rar should expose a page visual");

    assert_eq!(preview.kind, PreviewKind::Comic);
    assert_eq!(preview.detail.as_deref(), Some("Comic RAR archive"));
    assert_eq!(visual.kind, PreviewVisualKind::PageImage);
    assert_eq!(visual.layout, PreviewVisualLayout::FullHeight);
    assert_eq!(
        preview.navigation_position.as_ref().map(|position| (
            position.label,
            position.index,
            position.count
        )),
        Some(("Page", 0, 2))
    );
    assert!(visual.path.exists());
    assert!(line_texts.is_empty());

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn tar_preview_lists_inner_archive_contents() {
    let root = temp_path("tar-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("bundle.tar");
    write_tar_entries(
        &path,
        &[
            ("docs/readme.txt", "hello"),
            ("src/main.rs", "fn main() {}\n"),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("TAR archive"));
    assert!(line_texts.iter().any(|text| text.contains("docs/")));
    assert!(line_texts.iter().any(|text| text.contains("src/")));
    assert!(line_texts.iter().any(|text| text.contains("readme.txt")));
    assert!(line_texts.iter().any(|text| text.contains("main.rs")));
    assert!(!line_texts.iter().any(|text| text.contains("bundle.tar")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn tar_gz_preview_lists_inner_archive_contents() {
    let root = temp_path("tar-gz-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("bundle.tar.gz");
    write_tar_gz_entries(
        &path,
        &[
            ("docs/readme.txt", "hello"),
            ("src/main.rs", "fn main() {}\n"),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("TAR.GZ archive"));
    assert!(line_texts.iter().any(|text| text.contains("docs/")));
    assert!(line_texts.iter().any(|text| text.contains("src/")));
    assert!(line_texts.iter().any(|text| text.contains("readme.txt")));
    assert!(line_texts.iter().any(|text| text.contains("main.rs")));
    assert!(!line_texts.iter().any(|text| text.contains("bundle.tar")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn tgz_preview_keeps_tar_gz_label_and_contents_tree() {
    let root = temp_path("tgz-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("bundle.tgz");
    write_tar_gz_entries(
        &path,
        &[("assets/logo.txt", "logo"), ("bin/elio", "#!/bin/sh\n")],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("TAR.GZ archive"));
    assert!(line_texts.iter().any(|text| text.contains("assets/")));
    assert!(line_texts.iter().any(|text| text.contains("bin/")));
    assert!(line_texts.iter().any(|text| text.contains("logo.txt")));
    assert!(line_texts.iter().any(|text| text.contains("elio")));
    assert!(!line_texts.iter().any(|text| text.contains("bundle.tar")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn raw_xz_preview_uses_compressed_disk_image_label() {
    let root = temp_path("raw-xz-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("fedora.aarch64.raw.xz");
    if !write_xz_compressed_file(&path, b"raw-disk-image") {
        fs::remove_dir_all(root).expect("failed to remove temp root");
        return;
    }

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(
        preview.detail.as_deref(),
        Some("XZ-compressed raw disk image")
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Format") && (text.contains("XZ") || text.contains("xz")))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("fedora.aarch64.raw"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn pe_preview_shows_windows_executable_metadata() {
    let root = temp_path("pe-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("setup.exe");
    fs::write(&path, sample_pe_exe_bytes()).expect("failed to write pe fixture");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Binary);
    assert_eq!(preview.detail.as_deref(), Some("Windows executable"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("PE/COFF"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Architecture") && text.contains("x86_64"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Bits") && text.contains("64-bit"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Subsystem") && text.contains("Console"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Entry Point") && text.contains("0x1230"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn elf_preview_detects_binaries_without_extension() {
    let root = temp_path("elf-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("app-bin");
    fs::write(&path, sample_elf_shared_object_bytes()).expect("failed to write elf fixture");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Binary);
    assert_eq!(preview.detail.as_deref(), Some("ELF shared object"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Architecture") && text.contains("AArch64"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("ABI") && text.contains("Linux"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Entry Point") && text.contains("0x401000"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Sections") && text.contains("18"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn macho_preview_shows_dynamic_library_metadata() {
    let root = temp_path("macho-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("libelio.dylib");
    fs::write(&path, sample_macho_dylib_bytes()).expect("failed to write macho fixture");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Binary);
    assert_eq!(preview.detail.as_deref(), Some("Dynamic library"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("Mach-O"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Kind") && text.contains("Dynamic library"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Architecture") && text.contains("ARM64"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Load Commands") && text.contains("12"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn dos_mz_preview_falls_back_to_legacy_executable_metadata() {
    let root = temp_path("dos-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("legacy.bin");
    fs::write(&path, sample_dos_mz_bytes()).expect("failed to write dos fixture");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Binary);
    assert_eq!(preview.detail.as_deref(), Some("DOS executable"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("MZ"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Bits") && text.contains("16-bit"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn fat_macho_preview_lists_architectures_for_universal_binaries() {
    let root = temp_path("fat-macho-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("elio-universal");
    fs::write(&path, sample_macho_fat_bytes()).expect("failed to write fat macho fixture");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Binary);
    assert_eq!(preview.detail.as_deref(), Some("Mach-O universal binary"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("Mach-O (fat)"))
    );
    assert!(line_texts.iter().any(|text| {
        text.contains("Architecture") && text.contains("x86") && text.contains("ARM64")
    }));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Sections") && text.contains("2"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn jar_preview_surfaces_manifest_metadata() {
    let root = temp_path("jar-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("app.jar");
    write_zip_entries(
        &path,
        &[
            (
                "META-INF/MANIFEST.MF",
                "Implementation-Title: Elio\nImplementation-Version: 1.2.3\nMain-Class: elio.Main\nCreated-By: OpenJDK\n",
            ),
            ("elio/Main.class", "compiled"),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("Java archive"));
    assert!(line_texts.iter().any(|text| text == "Manifest"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Title") && text.contains("Elio"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Version") && text.contains("1.2.3"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Main-Class") && text.contains("elio.Main"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn typescript_preview_uses_code_renderer() {
    let root = temp_path("typescript");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.ts");
    fs::write(&path, "export const value: number = 1;\n").expect("failed to write ts");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("TypeScript"))
    );
    assert_ne!(
        span_color(&preview.lines[0], "export"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[0], "const"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn tsx_preview_uses_code_renderer() {
    let root = temp_path("tsx");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("App.tsx");
    fs::write(
        &path,
        "export function App() { return <div>Hello</div>; }\n",
    )
    .expect("failed to write tsx");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("TSX"))
    );
    assert_ne!(
        span_color(&preview.lines[0], "export"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[0], "return"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cargo_lock_preview_uses_code_renderer() {
    let root = temp_path("cargo-lock");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("Cargo.lock");
    fs::write(&path, "version = 3\n").expect("failed to write cargo lock");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("TOML"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn json_preview_formats_minified_content() {
    let root = temp_path("json");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("package.json");
    fs::write(&path, "{\"name\":\"elio\",\"nested\":{\"enabled\":true}}\n")
        .expect("failed to write json");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("JSON"));
    assert_eq!(preview.source_lines, Some(1));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("nested"))
    );
    assert!(preview.lines.len() > 1);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn json_preview_adds_root_summary_and_array_indexes() {
    let root = temp_path("json-summary");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("data.json");
    fs::write(&path, "{\"items\":[{\"id\":1},{\"id\":2}],\"ok\":true}\n")
        .expect("failed to write json");

    let preview = build_preview(&file_entry(path));
    let rendered = preview
        .lines
        .iter()
        .map(Line::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("root: object"));
    assert!(rendered.contains("2 keys"));
    assert!(rendered.contains("[0]: {id: 1}"));
    assert!(rendered.contains("[1]: {id: 2}"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn json_preview_inlines_small_scalar_structures() {
    let root = temp_path("json-inline");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("data.json");
    fs::write(
        &path,
        "{\"meta\":{\"id\":1,\"env\":\"dev\"},\"ports\":[80,443]}\n",
    )
    .expect("failed to write json");

    let preview = build_preview(&file_entry(path));
    let rendered = preview
        .lines
        .iter()
        .map(Line::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("meta: {env: \"dev\", id: 1}"));
    assert!(rendered.contains("ports: [80, 443]"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn json_preview_truncates_long_strings_with_length_hint() {
    let root = temp_path("json-long-string");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("data.json");
    fs::write(&path, format!("{{\"token\":\"{}\"}}\n", "a".repeat(120)))
        .expect("failed to write json");

    let preview = build_preview(&file_entry(path));
    let rendered = preview
        .lines
        .iter()
        .map(Line::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("token: "));
    assert!(rendered.contains("(120 chars)"));
    assert!(rendered.contains("…"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn truncated_json_preview_reports_why_formatting_was_skipped() {
    let root = temp_path("json-truncated");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("package.json");
    let oversized = format!("{{\"value\":\"{}\"}}", "a".repeat(PREVIEW_LIMIT_BYTES));
    fs::write(&path, oversized).expect("failed to write oversized json");

    let preview = build_preview(&file_entry(path));
    let header = preview
        .header_detail(0, 12)
        .expect("formatted header detail should be present");

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        header.contains("formatted preview unavailable for partial file"),
        "unexpected header: {header}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn dotenv_preview_uses_structured_renderer() {
    let root = temp_path("dotenv");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join(".env.local");
    fs::write(&path, "APP_ENV=dev\nPORT=3000\n").expect("failed to write dotenv file");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail == ".env")
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("APP_ENV"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn jsonc_preview_uses_structured_renderer() {
    let root = temp_path("jsonc");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("deno.jsonc");
    fs::write(&path, "{\n  // comment\n  \"name\": \"elio\",\n}\n").expect("failed to write jsonc");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("JSONC"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("name"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn json5_preview_uses_structured_renderer() {
    let root = temp_path("json5");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("config.json5");
    fs::write(&path, "{\n  trailing: true,\n  list: [1, 2,],\n}\n").expect("failed to write json5");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("JSON5"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("trailing"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn yaml_preview_uses_structured_renderer() {
    let root = temp_path("yaml");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("docker-compose.yaml");
    fs::write(
        &path,
        "services:\n  app:\n    image: elio:latest\n    ports:\n      - \"3000:3000\"\n",
    )
    .expect("failed to write yaml");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("YAML"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("services"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn text_preview_stays_plain() {
    let root = temp_path("text");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("notes.txt");
    fs::write(&path, "hello\nworld\n").expect("failed to write text");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Text);
    assert_eq!(preview.lines[0].spans.len(), 1);
    assert_eq!(preview.lines[0].spans[0].content, "hello");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn text_preview_keeps_enough_lines_for_scrolling() {
    let root = temp_path("scroll-depth");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("long.txt");
    let text = (1..=80)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&path, text).expect("failed to write long text");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Text);
    assert!(preview.lines.len() >= 80);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn utf8_preview_trims_to_last_valid_boundary() {
    let root = temp_path("utf8-boundary");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("unicode.txt");
    let bytes = [
        "a".repeat(PREVIEW_LIMIT_BYTES - 1).into_bytes(),
        "é".as_bytes().to_vec(),
    ]
    .concat();
    fs::write(&path, bytes).expect("failed to write unicode text");

    let preview = read_text_preview(&path)
        .expect("preview read should succeed")
        .expect("utf8 text should stay text");

    assert!(preview.bytes_truncated);
    assert_eq!(preview.text.len(), PREVIEW_LIMIT_BYTES - 1);
    assert!(preview.text.chars().all(|ch| ch == 'a'));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn code_preview_sanitizes_control_characters() {
    let root = temp_path("control-char-code");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.c");
    let contents = "int main(void) {\n    puts(\"hello \u{1b} world\");\n    return 0;\n}\n";
    fs::write(&path, contents).expect("failed to write control-char source");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    assert!(
        line_texts.iter().any(|line| line.contains("^[ world")),
        "expected control characters to be rendered safely, got: {line_texts:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn utf8_text_file_is_not_mislabeled_as_binary() {
    let root = temp_path("utf8-text-kind");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("unicode.txt");
    let bytes = [
        "a".repeat(PREVIEW_LIMIT_BYTES - 1).into_bytes(),
        "é".as_bytes().to_vec(),
    ]
    .concat();
    fs::write(&path, bytes).expect("failed to write unicode text");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Text);
    assert!(preview.truncated);
    assert!(preview.lines.iter().all(|line| {
        line.spans
            .iter()
            .all(|span| span.content != "No text preview available")
    }));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn utf16le_bom_text_file_is_not_mislabeled_as_binary() {
    let root = temp_path("utf16le-text-kind");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("unicode.txt");
    let text = "Thu Jan 15 21:36:25 2026\r\nHello from UTF-16\r\n";
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    fs::write(&path, bytes).expect("failed to write utf16 text");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_ne!(preview.kind, PreviewKind::Binary);
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Hello from UTF-16"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn raster_image_preview_uses_image_metadata_fallback() {
    let root = temp_path("image-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("cover.png");
    write_test_raster_image(&path, ImageFormat::Png, 600, 300);

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Image);
    assert_eq!(preview.detail.as_deref(), Some("PNG image"));
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Dimensions") && line.contains("600x300"))
    );
    assert!(
        line_texts
            .iter()
            .all(|line| !line.contains("Binary or unsupported file"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn extensionless_png_preview_uses_image_metadata_fallback() {
    let root = temp_path("image-metadata-noext");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("background");
    write_test_raster_image(&path, ImageFormat::Png, 600, 300);

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Image);
    assert_eq!(preview.detail.as_deref(), Some("PNG image"));
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Dimensions") && line.contains("600x300"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn utf16_log_preview_uses_decoded_text() {
    let root = temp_path("utf16-log");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("socialclub.log");
    let text = "[00000000] Thu Jan 15 21:36:25 2026 INFO launcher started\r\n\
             [00000001] Thu Jan 15 21:36:26 2026 ERROR request_id=42 failed\r\n";
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    fs::write(&path, bytes).expect("failed to write utf16 log");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_ne!(preview.kind, PreviewKind::Binary);
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("launcher started") || line.contains("request_id=42"))
    );
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Log"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn byte_truncated_preview_reports_truncation_without_fake_line_totals() {
    let root = temp_path("byte-truncated");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("notes.txt");
    fs::write(&path, "a".repeat(PREVIEW_LIMIT_BYTES + 32)).expect("failed to write text");

    let preview = build_preview(&file_entry(path));
    let header = preview
        .header_detail(0, 20)
        .expect("header detail should be present");

    assert_eq!(preview.kind, PreviewKind::Text);
    assert!(preview.truncated);
    assert!(preview.source_lines.is_none());
    assert!(header.contains("truncated to 64 KiB"));
    assert!(!header.contains("lines"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn line_truncated_preview_reports_visible_limit() {
    let root = temp_path("line-truncated");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("long.txt");
    let text = (1..=300)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&path, text).expect("failed to write long text");

    let preview = build_preview(&file_entry(path));
    let header = preview
        .header_detail(0, 20)
        .expect("header detail should be present");

    assert!(preview.truncated);
    assert_eq!(preview.source_lines, Some(300));
    assert!(header.contains("300 lines"));
    assert!(header.contains("showing first 240 lines"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn code_preview_respects_custom_line_limit() {
    let root = temp_path("code-line-limit");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.rs");
    let text = (1..=12)
        .map(|index| format!("let value_{index} = {index};"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&path, text).expect("failed to write code");

    let preview = build_preview_with_options_and_code_line_limit(
        &file_entry(path),
        &PreviewRequestOptions::Default,
        4,
        &|| false,
    );
    let header = preview
        .header_detail(0, 20)
        .expect("header detail should be present");

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.lines.len(), 4);
    assert_eq!(
        preview.line_coverage.map(|coverage| coverage.shown_lines),
        Some(4)
    );
    assert!(
        header.contains("showing first 4 lines"),
        "unexpected header: {header}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
#[cfg(unix)]
fn protected_directory_preview_reports_permission_denied() {
    let root = temp_path("protected-dir-preview");
    let locked = root.join("locked");
    fs::create_dir_all(&locked).expect("failed to create locked dir");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("failed to lock dir");

    let preview = build_preview(&directory_entry(locked.clone()));

    assert_eq!(preview.kind, PreviewKind::Unavailable);
    assert_eq!(preview.detail.as_deref(), Some("Permission denied"));
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line_text(line).contains("permission"))
    );

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("failed to unlock dir");
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
#[cfg(unix)]
fn protected_file_preview_reports_permission_denied() {
    let root = temp_path("protected-file-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("secret.txt");
    fs::write(&path, "secret").expect("failed to write file");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).expect("failed to lock file");

    let preview = build_preview(&file_entry(path.clone()));

    assert_eq!(preview.kind, PreviewKind::Unavailable);
    assert_eq!(preview.detail.as_deref(), Some("Permission denied"));
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line_text(line).contains("permission"))
    );

    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("failed to unlock file");
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn license_file_with_hard_line_breaks_is_reflowed_into_paragraphs() {
    let root = temp_path("license-reflow");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("LICENSE");

    // Simulate a hard-wrapped license (lines at ~76 chars, traditional terminal format).
    // Each paragraph is a block of consecutive lines, separated by blank lines.
    let contents = "\
MIT License

Copyright (c) 2024 Example Author

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the \"Software\"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.
";
    fs::write(&path, contents).expect("failed to write license");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Text);

    // After reflowing: the five-line permission-grant block should be ONE long line.
    let permission_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("Permission is hereby granted"))
        .expect("permission grant line should exist");
    let text = line_text(permission_line);
    // The reflowed line must contain the last part that was originally on a separate line.
    assert!(
        text.contains("furnished to do so"),
        "permission grant should be reflowed into a single line, got: {text:?}"
    );

    // Blank-line paragraph separators must be preserved.
    let blank_count = preview.lines.iter().filter(|l| l.spans.is_empty()).count();
    assert!(
        blank_count >= 3,
        "blank paragraph separators should be preserved, got {blank_count}"
    );

    // The reflowed preview must have far fewer lines than the source (paragraphs, not raw lines).
    let source_lines = contents.lines().count();
    assert!(
        preview.lines.len() < source_lines,
        "reflowed output ({} lines) should be shorter than source ({source_lines} lines)",
        preview.lines.len()
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
