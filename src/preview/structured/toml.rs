use super::StructuredPreview;
use crate::{file_info::HighlightLanguage, preview::highlighting};
use toml_edit::DocumentMut;

pub(super) fn render_toml_preview(text: &str, detail: &'static str) -> Option<StructuredPreview> {
    let document = text.parse::<DocumentMut>().ok()?;
    let rendered = document.to_string();
    Some(StructuredPreview {
        lines: highlighting::render_code_preview(&rendered, Some(HighlightLanguage::Toml), false),
        detail,
        truncation_note: None,
    })
}
