use super::super::{
    common::{humanize_pdfinfo_datetime, present_str, push_count_stat, push_metadata_field},
    metadata::DocumentMetadata,
};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path, process::Command};

pub(super) fn extract_pdf_metadata(path: &Path) -> Option<DocumentMetadata> {
    let mut bytes = Vec::with_capacity(256);
    File::open(path)
        .ok()?
        .take(256)
        .read_to_end(&mut bytes)
        .ok()?;

    let mut metadata = DocumentMetadata::default();
    if let Some(version) = parse_pdf_version(&bytes) {
        metadata.variant = Some(format!("PDF {version}"));
        metadata
            .metadata
            .push(("PDF Version".to_string(), version.to_string()));
    }

    let output = Command::new("pdfinfo").arg(path).output().ok();
    let Some(output) = output.filter(|output| output.status.success()) else {
        return Some(metadata);
    };
    let fields = parse_pdfinfo_fields(&String::from_utf8_lossy(&output.stdout));
    metadata.title = fields
        .get("Title")
        .and_then(|value| present_str(value, "Title"));
    metadata.subject = fields
        .get("Subject")
        .and_then(|value| present_str(value, "Subject"));
    metadata.author = fields
        .get("Author")
        .and_then(|value| present_str(value, "Author"));
    metadata.application = fields
        .get("Creator")
        .and_then(|value| present_str(value, "Application"));
    metadata.created = fields
        .get("CreationDate")
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(humanize_pdfinfo_datetime);
    metadata.modified = fields
        .get("ModDate")
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(humanize_pdfinfo_datetime);

    push_count_stat(
        &mut metadata,
        "Pages",
        fields
            .get("Pages")
            .and_then(|value| value.trim().parse().ok()),
    );
    push_metadata_field(
        &mut metadata,
        "Producer",
        fields
            .get("Producer")
            .and_then(|value| present_str(value, "Producer")),
    );
    push_metadata_field(
        &mut metadata,
        "Page Size",
        fields
            .get("Page size")
            .and_then(|value| present_str(value, "Page size")),
    );
    push_metadata_field(
        &mut metadata,
        "Tagged",
        fields
            .get("Tagged")
            .and_then(|value| present_str(value, "Tagged")),
    );
    push_metadata_field(
        &mut metadata,
        "Encrypted",
        fields
            .get("Encrypted")
            .and_then(|value| present_str(value, "Encrypted")),
    );
    push_metadata_field(
        &mut metadata,
        "Optimized",
        fields
            .get("Optimized")
            .and_then(|value| present_str(value, "Optimized")),
    );

    Some(metadata)
}

fn parse_pdfinfo_fields(output: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    for line in output.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        fields.insert(key.to_string(), value.to_string());
    }
    fields
}

fn parse_pdf_version(bytes: &[u8]) -> Option<&str> {
    // Scan only the first line; PDFs commonly include binary sentinel bytes
    // on the second line (e.g. `%âãÏÓ`) that would make the full buffer
    // invalid UTF-8.
    let first_line_end = bytes
        .iter()
        .position(|&b| b == b'\n' || b == b'\r')
        .unwrap_or(bytes.len());
    let first_line = std::str::from_utf8(&bytes[..first_line_end]).ok()?;
    first_line.trim().strip_prefix("%PDF-")
}
