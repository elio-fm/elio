use super::StructuredPreview;
use crate::{
    file_info::{CodeBackend, CustomCodeKind, PreviewSpec},
    preview,
};
use toml_edit::DocumentMut;

pub(super) fn render_toml_preview(text: &str, detail: &'static str) -> Option<StructuredPreview> {
    let document = text.parse::<DocumentMut>().ok()?;
    let rendered = document.to_string();
    Some(StructuredPreview {
        lines: preview::code::render_code_preview(
            PreviewSpec::code("toml", CodeBackend::Custom(CustomCodeKind::Toml), None),
            &rendered,
            false,
            preview::default_code_preview_line_limit(),
            &|| false,
        ),
        detail,
        truncation_note: None,
    })
}
