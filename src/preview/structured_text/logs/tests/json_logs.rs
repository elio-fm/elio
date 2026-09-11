use super::super::render_log_preview;

fn rendered_preview(text: &str) -> String {
    render_log_preview(text)
        .expect("structured log preview should render")
        .lines
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn json_logs_render_as_structured_entries() {
    let rendered = rendered_preview(
        "{\"timestamp\":\"2026-03-10T12:00:00Z\",\"level\":\"info\",\"message\":\"started\",\"service\":\"api\"}\n\
             {\"timestamp\":\"2026-03-10T12:00:01Z\",\"level\":\"error\",\"message\":\"failed\",\"request_id\":42}\n",
    );

    assert!(rendered.contains("JSON lines"));
    assert!(rendered.contains("started"));
    assert!(rendered.contains("service"));
    assert!(rendered.contains("ERROR"));
}

#[test]
fn json_logs_accept_alias_fields_and_stringify_nested_values() {
    let rendered = rendered_preview(
        "{\"@timestamp\":\"2026-03-10T12:00:00Z\",\"severity\":\"warning\",\"summary\":\"cache miss\",\"http\":{\"path\":\"/login\",\"status\":404}}\n",
    );

    assert!(rendered.contains("JSON lines"));
    assert!(rendered.contains("WARN"));
    assert!(rendered.contains("cache miss"));
    assert!(rendered.contains("http"));
    assert!(rendered.contains("/login"));
}
