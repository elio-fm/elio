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
    fn json_logs_render_as_structured_entries() {
        let preview = render_log_preview(
            "{\"timestamp\":\"2026-03-10T12:00:00Z\",\"level\":\"info\",\"message\":\"started\",\"service\":\"api\"}\n\
             {\"timestamp\":\"2026-03-10T12:00:01Z\",\"level\":\"error\",\"message\":\"failed\",\"request_id\":42}\n",
        )
        .expect("json logs should render");

        let rendered = preview
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("JSON lines"));
        assert!(rendered.contains("started"));
        assert!(rendered.contains("service"));
        assert!(rendered.contains("ERROR"));
    }

    #[test]
    fn access_logs_are_detected_and_summarized() {
        let preview = render_log_preview(
            "127.0.0.1 - - [10/Mar/2026:12:00:00 +0000] \"GET /login HTTP/1.1\" 500 123 \"-\" \"curl/8.0\"\n",
        )
        .expect("access log should render");

        let rendered = preview
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Access log"));
        assert!(rendered.contains("GET /login"));
        assert!(rendered.contains("status"));
        assert!(rendered.contains("500"));
    }

    #[test]
    fn access_logs_keep_optional_referer_and_user_agent_fields() {
        let preview = render_log_preview(
            "127.0.0.1 app elio [10/Mar/2026:12:00:00 +0000] \"GET /login HTTP/1.1\" 404 321 \"https://elio.dev/docs\" \"Mozilla/5.0\"\n",
        )
        .expect("access log with optional fields should render");

        let rendered = preview
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("WARN"));
        assert!(rendered.contains("ident"));
        assert!(rendered.contains("elio"));
        assert!(rendered.contains("referer"));
        assert!(rendered.contains("https://elio.dev/docs"));
        assert!(rendered.contains("user-agent"));
        assert!(rendered.contains("Mozilla/5.0"));
    }

    #[test]
    fn json_logs_accept_alias_fields_and_stringify_nested_values() {
        let preview = render_log_preview(
            "{\"@timestamp\":\"2026-03-10T12:00:00Z\",\"severity\":\"warning\",\"summary\":\"cache miss\",\"http\":{\"path\":\"/login\",\"status\":404}}\n",
        )
        .expect("json alias fields should render");

        let rendered = preview
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("JSON lines"));
        assert!(rendered.contains("WARN"));
        assert!(rendered.contains("cache miss"));
        assert!(rendered.contains("http"));
        assert!(rendered.contains("/login"));
    }

    #[test]
    fn multiline_logs_keep_stack_traces_attached() {
        let preview = render_log_preview(
            "2026-03-10T12:00:00Z ERROR request_id=42 msg=\"request failed\"\n\
                at service.handle (/srv/app.js:10)\n\
                Caused by: timeout\n\
             2026-03-10T12:00:01Z INFO request_id=42 recovered\n",
        )
        .expect("multiline log should render");

        let rendered = preview
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("request failed"));
        assert!(rendered.contains("Caused by: timeout"));
        assert!(rendered.contains("recovered"));
    }

    #[test]
    fn lower_case_and_bracketed_levels_are_normalized() {
        let preview = render_log_preview(
            "2026-03-10 12:00:00 [warn] request_id=42 delayed\n\
             2026-03-10 12:00:01 level=error request_id=42 failed\n",
        )
        .expect("normalized levels should render");

        let rendered = preview
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("WARN"));
        assert!(rendered.contains("ERROR"));
    }

    #[test]
    fn general_logs_preserve_quoted_field_values_and_month_timestamps() {
        let preview = render_log_preview(
            "Mar 10 12:00:00 level=info request_id=42 msg=\"cache rebuilt successfully\"\n",
        )
        .expect("general log with quoted fields should render");

        let rendered = preview
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Application log"));
        assert!(rendered.contains("Mar 10 12:00:00"));
        assert!(rendered.contains("INFO"));
        assert!(rendered.contains("request_id"));
        assert!(rendered.contains("cache rebuilt successfully"));
    }

    #[test]
    fn unstructured_logs_return_none_for_structured_rendering() {
        assert!(
            render_log_preview("starting application\nloading configuration\nready\n").is_none()
        );
    }
}
