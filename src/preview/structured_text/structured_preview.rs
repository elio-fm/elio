use super::{dotenv, json_yaml, logs, toml};
use crate::file_classification::StructuredFormat;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

pub(in crate::preview) struct StructuredPreviewAttempt {
    pub preview: Option<StructuredPreview>,
    pub note: Option<String>,
}

pub(in crate::preview) struct StructuredPreview {
    pub lines: Vec<Line<'static>>,
    pub detail: &'static str,
    pub truncation_note: Option<String>,
}

pub(in crate::preview) const LINE_LIMIT: usize = super::super::PREVIEW_RENDER_LINE_LIMIT;

pub(in crate::preview) fn render_structured_preview(
    text: &str,
    format: StructuredFormat,
    input_truncated: bool,
) -> StructuredPreviewAttempt {
    if input_truncated {
        return StructuredPreviewAttempt {
            preview: None,
            note: Some("formatted preview unavailable for partial file".to_string()),
        };
    }

    let preview = match format {
        StructuredFormat::Json => json_yaml::render_json_preview(text, format.detail_label()),
        StructuredFormat::Jsonc | StructuredFormat::Json5 => {
            json_yaml::render_json5_preview(text, format.detail_label())
        }
        StructuredFormat::Toml => toml::render_toml_preview(text, format.detail_label()),
        StructuredFormat::Yaml => json_yaml::render_yaml_preview(text, format.detail_label()),
        StructuredFormat::Dotenv => Some(dotenv::render_dotenv_preview(text)),
        StructuredFormat::Log => logs::render_log_preview(text),
    };

    StructuredPreviewAttempt {
        preview,
        note: None,
    }
}

pub(in crate::preview) fn styled(
    text: &str,
    color: ratatui::style::Color,
    modifier: Modifier,
) -> Span<'static> {
    Span::styled(
        text.to_string(),
        Style::default().fg(color).add_modifier(modifier),
    )
}

#[cfg(test)]
#[path = "tests/structured_preview.rs"]
mod tests;
