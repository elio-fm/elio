use super::super::render_log_preview;

fn rendered_preview(text: &str) -> String {
    render_log_preview(text)
        .expect("access log preview should render")
        .lines
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn access_logs_are_detected_and_summarized() {
    let rendered = rendered_preview(
        "127.0.0.1 - - [10/Mar/2026:12:00:00 +0000] \"GET /login HTTP/1.1\" 500 123 \"-\" \"curl/8.0\"\n",
    );

    assert!(rendered.contains("Access log"));
    assert!(rendered.contains("GET /login"));
    assert!(rendered.contains("status"));
    assert!(rendered.contains("500"));
}

#[test]
fn access_logs_keep_optional_referer_and_user_agent_fields() {
    let rendered = rendered_preview(
        "127.0.0.1 app elio [10/Mar/2026:12:00:00 +0000] \"GET /login HTTP/1.1\" 404 321 \"https://elio.dev/docs\" \"Mozilla/5.0\"\n",
    );

    assert!(rendered.contains("WARN"));
    assert!(rendered.contains("ident"));
    assert!(rendered.contains("elio"));
    assert!(rendered.contains("referer"));
    assert!(rendered.contains("https://elio.dev/docs"));
    assert!(rendered.contains("user-agent"));
    assert!(rendered.contains("Mozilla/5.0"));
}
