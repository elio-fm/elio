use super::super::render_log_preview;

fn rendered_preview(text: &str) -> String {
    render_log_preview(text)
        .expect("general log preview should render")
        .lines
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn multiline_logs_keep_stack_traces_attached() {
    let rendered = rendered_preview(
        "2026-03-10T12:00:00Z ERROR request_id=42 msg=\"request failed\"\n\
                at service.handle (/srv/app.js:10)\n\
                Caused by: timeout\n\
             2026-03-10T12:00:01Z INFO request_id=42 recovered\n",
    );

    assert!(rendered.contains("request failed"));
    assert!(rendered.contains("Caused by: timeout"));
    assert!(rendered.contains("recovered"));
}

#[test]
fn lower_case_and_bracketed_levels_are_normalized() {
    let rendered = rendered_preview(
        "2026-03-10 12:00:00 [warn] request_id=42 delayed\n\
             2026-03-10 12:00:01 level=error request_id=42 failed\n",
    );

    assert!(rendered.contains("WARN"));
    assert!(rendered.contains("ERROR"));
}

#[test]
fn general_logs_preserve_quoted_field_values_and_month_timestamps() {
    let rendered = rendered_preview(
        "Mar 10 12:00:00 level=info request_id=42 msg=\"cache rebuilt successfully\"\n",
    );

    assert!(rendered.contains("Application log"));
    assert!(rendered.contains("Mar 10 12:00:00"));
    assert!(rendered.contains("INFO"));
    assert!(rendered.contains("request_id"));
    assert!(rendered.contains("cache rebuilt successfully"));
}
