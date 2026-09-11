use super::*;

#[test]
fn jsonc_structured_preview_accepts_comments_and_trailing_commas() {
    let attempt = render_structured_preview(
        "{\n  // comment\n  \"name\": \"elio\",\n}\n",
        StructuredFormat::Jsonc,
        false,
    );

    let preview = attempt.preview.expect("jsonc should render");
    assert_eq!(preview.detail, "JSONC");
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("name"))
    );
}

#[test]
fn dotenv_structured_preview_aligns_bindings() {
    let attempt =
        render_structured_preview("APP_ENV=dev\nPORT=3000\n", StructuredFormat::Dotenv, false);

    let preview = attempt.preview.expect("dotenv should render");
    assert_eq!(preview.detail, ".env");
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("APP_ENV"))
    );
}

#[test]
fn toml_structured_preview_keeps_toml_shape() {
    let attempt = render_structured_preview(
        "[package]\nname = \"elio\"\nversion = \"0.1.0\"\n\n[dependencies]\nanyhow = \"1.0\"\nimage = { version = \"0.25\", default-features = false }\n\n[profile.release]\nstrip = true\n",
        StructuredFormat::Toml,
        false,
    );

    let preview = attempt.preview.expect("toml should render");
    assert_eq!(preview.detail, "TOML");

    let lines = preview
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>();

    let package_index = lines
        .iter()
        .position(|line| line.contains("[package]"))
        .expect("package header should be present");
    let dependencies_index = lines
        .iter()
        .position(|line| line.contains("[dependencies]"))
        .expect("dependencies header should be present");
    let profile_index = lines
        .iter()
        .position(|line| line.contains("[profile.release]"))
        .expect("profile header should be present");

    assert!(package_index < dependencies_index);
    assert!(dependencies_index < profile_index);
    assert!(lines.iter().any(|line| line.contains("name = \"elio\"")));
    assert!(lines.iter().any(|line| line.contains("anyhow = \"1.0\"")));
    assert!(
        lines
            .iter()
            .any(|line| line.contains("image = { version = \"0.25\""))
    );
    assert!(!lines.iter().any(|line| line.contains("root: object")));
}

#[test]
fn log_structured_preview_includes_level_summary() {
    let attempt = render_structured_preview(
        "2026-03-10T12:00:00Z ERROR request_id=42 path=/login failed\n",
        StructuredFormat::Log,
        false,
    );

    let preview = attempt.preview.expect("log should render");
    assert_eq!(preview.detail, "Log");
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("ERROR"))
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("request_id"))
    );
}
