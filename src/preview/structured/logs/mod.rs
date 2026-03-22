mod access;
mod general;
mod json;
mod render;
mod tokenize;
mod types;

use self::access::parse_access_log_document;
use self::general::parse_general_log_document;
use self::json::parse_json_log_document;
use self::render::render_parsed_log;
use super::StructuredPreview;

pub(super) fn render_log_preview(text: &str) -> Option<StructuredPreview> {
    if text.trim().is_empty() {
        return Some(StructuredPreview {
            lines: vec![ratatui::text::Line::from("File is empty")],
            detail: crate::file_info::StructuredFormat::Log.detail_label(),
            truncation_note: None,
        });
    }

    let parsed = parse_json_log_document(text)
        .or_else(|| parse_access_log_document(text))
        .or_else(|| parse_general_log_document(text))?;
    Some(render_parsed_log(parsed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unstructured_logs_return_none_for_structured_rendering() {
        assert!(
            render_log_preview("starting application\nloading configuration\nready\n").is_none()
        );
    }
}
