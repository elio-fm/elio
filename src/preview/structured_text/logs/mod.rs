mod access_logs;
mod application_logs;
mod json_logs;
mod log_rendering;
mod log_types;

use self::access_logs::parse_access_log_document;
use self::application_logs::parse_application_log_document;
use self::json_logs::parse_json_log_document;
use self::log_rendering::render_parsed_log;
use super::StructuredPreview;

pub(super) fn render_log_preview(text: &str) -> Option<StructuredPreview> {
    if text.trim().is_empty() {
        return Some(StructuredPreview {
            lines: vec![ratatui::text::Line::from("File is empty")],
            detail: crate::file_classification::StructuredFormat::Log.detail_label(),
            truncation_note: None,
        });
    }

    let parsed = parse_json_log_document(text)
        .or_else(|| parse_access_log_document(text))
        .or_else(|| parse_application_log_document(text))?;
    Some(render_parsed_log(parsed))
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
