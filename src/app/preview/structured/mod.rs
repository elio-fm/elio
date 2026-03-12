mod kv;
mod logs;
mod tree;

use crate::file_facts::StructuredFormat;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

pub(super) struct StructuredPreviewAttempt {
    pub preview: Option<StructuredPreview>,
    pub note: Option<String>,
}

pub(super) struct StructuredPreview {
    pub lines: Vec<Line<'static>>,
    pub detail: &'static str,
    pub truncation_note: Option<String>,
}

pub(super) const LINE_LIMIT: usize = super::PREVIEW_RENDER_LINE_LIMIT;

pub(super) fn render_structured_preview(
    text: &str,
    format: StructuredFormat,
    input_truncated: bool,
) -> StructuredPreviewAttempt {
    if input_truncated {
        return StructuredPreviewAttempt {
            preview: None,
            note: Some("structured preview skipped: input truncated".to_string()),
        };
    }

    let preview = match format {
        StructuredFormat::Json => tree::render_json_preview(text, format.detail_label()),
        StructuredFormat::Jsonc | StructuredFormat::Json5 => {
            tree::render_json5_preview(text, format.detail_label())
        }
        StructuredFormat::Toml => tree::render_toml_preview(text, format.detail_label()),
        StructuredFormat::Yaml => tree::render_yaml_preview(text, format.detail_label()),
        StructuredFormat::Dotenv => Some(kv::render_dotenv_preview(text)),
        StructuredFormat::Log => logs::render_log_preview(text),
    };

    StructuredPreviewAttempt {
        preview,
        note: None,
    }
}

pub(super) fn styled(
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
mod tests {
    use super::*;

    #[test]
    fn jsonc_structured_preview_accepts_comments_and_trailing_commas() {
        let attempt = render_structured_preview(
            "{\n  // comment\n  \"name\": \"elio\",\n}\n",
            StructuredFormat::Jsonc,
            false,
        );

        let preview = attempt.preview.expect("jsonc should render");
        assert_eq!(preview.detail, "JSONC (structured)");
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("name")));
    }

    #[test]
    fn dotenv_structured_preview_aligns_bindings() {
        let attempt =
            render_structured_preview("APP_ENV=dev\nPORT=3000\n", StructuredFormat::Dotenv, false);

        let preview = attempt.preview.expect("dotenv should render");
        assert_eq!(preview.detail, ".env (structured)");
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("APP_ENV")));
    }

    #[test]
    fn log_structured_preview_includes_level_summary() {
        let attempt = render_structured_preview(
            "2026-03-10T12:00:00Z ERROR request_id=42 path=/login failed\n",
            StructuredFormat::Log,
            false,
        );

        let preview = attempt.preview.expect("log should render");
        assert_eq!(preview.detail, "Log (structured)");
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("ERROR")));
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("request_id")));
    }
}
