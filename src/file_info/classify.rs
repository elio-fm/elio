use super::{
    FileFacts, PreviewSpec, archives::inspect_archive_name, extensions::inspect_extension,
    names::inspect_exact_name,
};
use crate::app::{EntryKind, FileClass};
use std::{fs::File, io::Read, path::Path};

pub(crate) fn inspect_path(path: &Path, kind: EntryKind) -> FileFacts {
    if kind == EntryKind::Directory {
        return FileFacts {
            builtin_class: FileClass::Directory,
            specific_type_label: None,
            preview: PreviewSpec::plain_text(),
        };
    }

    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(normalize_key)
        .unwrap_or_default();
    if let Some(facts) = inspect_exact_name(&name) {
        return facts;
    }
    if let Some(facts) = inspect_archive_name(&name) {
        return facts;
    }

    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(normalize_key)
        .unwrap_or_default();
    let facts = inspect_extension(&ext);
    if ext.is_empty() {
        sniff_extensionless_file_type(path).unwrap_or(facts)
    } else {
        facts
    }
}

fn normalize_key(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

fn sniff_extensionless_file_type(path: &Path) -> Option<FileFacts> {
    let mut file = File::open(path).ok()?;
    let mut buffer = [0_u8; 512];
    let bytes_read = file.read(&mut buffer).ok()?;
    sniff_image_type(&buffer[..bytes_read])
}

fn sniff_image_type(buffer: &[u8]) -> Option<FileFacts> {
    if buffer.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some(image_facts("PNG image"));
    }
    if buffer.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(image_facts("JPEG image"));
    }
    if buffer.starts_with(b"GIF87a") || buffer.starts_with(b"GIF89a") {
        return Some(image_facts("GIF image"));
    }
    if buffer.len() >= 12 && &buffer[..4] == b"RIFF" && &buffer[8..12] == b"WEBP" {
        return Some(image_facts("WebP image"));
    }

    let text = std::str::from_utf8(buffer).ok()?;
    let trimmed = text.trim_start_matches(|ch: char| ch.is_ascii_whitespace() || ch == '\u{feff}');
    if trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && trimmed.contains("<svg")) {
        return Some(image_facts("SVG image"));
    }

    None
}

fn image_facts(label: &'static str) -> FileFacts {
    FileFacts {
        builtin_class: FileClass::Image,
        specific_type_label: Some(label),
        preview: PreviewSpec::plain_text(),
    }
}
