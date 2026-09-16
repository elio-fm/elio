use super::*;
use crate::file_classification::{CodeBackend, CustomCodeKind, PreviewSpec};

#[test]
fn configured_tab_width_applies_across_preview_backends() {
    const WIDTH_ENV: &str = "ELIO_TEST_PREVIEW_TAB_WIDTH";
    let Ok(setting) = std::env::var(WIDTH_ENV) else {
        // Configuration is initialized once per process. Isolate each setting
        // so these checks cannot change configuration for other parallel tests.
        for setting in ["default", "-1", "0", "1", "2", "3", "4", "8", "16", "83294"] {
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    concat!(
                        module_path!(),
                        "::configured_tab_width_applies_across_preview_backends"
                    )
                    .trim_start_matches("elio::"),
                ])
                .env(WIDTH_ENV, setting)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "tab width {setting}:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        }
        return;
    };

    let root = temp_path("tab-width");
    fs::create_dir_all(&root).unwrap();
    let config_path = root.join("config.toml");
    let config = if setting == "default" {
        String::new()
    } else {
        format!("[preview]\ntab_width = {setting}\n")
    };
    fs::write(&config_path, config).unwrap();
    crate::config::initialize(Some(&config_path)).unwrap();

    let width = setting.parse::<i64>().unwrap_or(4).clamp(1, 16) as usize;
    let source = "\t\tvalue=\"界\tb\u{1b}\"";
    // Two leading tab stops, then nine display columns before the next tab.
    let expected = format!(
        "{}value=\"界{}b^[\"",
        " ".repeat(2 * width),
        " ".repeat(width - 9 % width),
    );

    // Plain text, Syntect, shell, and the built-in INI highlighter.
    for extension in ["txt", "py", "sh", "ini"] {
        let path = root.join(format!("sample.{extension}"));
        fs::write(&path, source).unwrap();
        let preview = build_preview(&file_entry(path));
        let prefix = if extension == "txt" { "" } else { "  1 " };
        assert_eq!(
            line_text(&preview.lines[0]),
            format!("{prefix}{expected}"),
            "{extension}"
        );
    }

    // Both number and marker gutters must be excluded from tab columns.
    // Repeat the source to verify that each source line starts at column zero.
    for (syntax, backend) in [
        ("plain", CodeBackend::Plain),
        ("python", CodeBackend::Syntect),
        ("sh", CodeBackend::Syntect),
        ("ini", CodeBackend::Custom(CustomCodeKind::Ini)),
    ] {
        for line_numbers in [false, true] {
            let lines = crate::preview::code::render_code_preview(
                PreviewSpec::code(syntax, backend, None),
                &format!("{source}\n{source}"),
                line_numbers,
                20,
                &|| false,
            );
            let preview = PreviewContent::new(PreviewKind::Code, lines);
            for (index, line) in preview.lines.iter().enumerate() {
                let prefix = if line_numbers {
                    format!("  {} ", index + 1)
                } else {
                    "│ ".into()
                };
                assert_eq!(line_text(line), format!("{prefix}{expected}"), "{syntax}");
            }
        }
    }

    // File names and other terminal text keep their existing tab handling.
    assert_eq!(
        crate::filesystem::sanitize_terminal_text("a\tb\u{1b}"),
        "a    b^["
    );
    fs::remove_dir_all(root).unwrap();
}
