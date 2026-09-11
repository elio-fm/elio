use super::*;

#[test]
fn desktop_preview_uses_code_renderer() {
    let root = temp_path("desktop-entry");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("app.desktop");
    fs::write(
        &path,
        "[Desktop Entry]\nName=エリオ\nName[ja]=エリオ\nExec=elio\nTerminal=false\n",
    )
    .expect("failed to write desktop entry");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail == "Desktop Entry")
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("エリオ"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn directive_conf_preview_is_used_for_ambiguous_conf() {
    let root = temp_path("directive-conf-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("custom.conf");
    fs::write(
        &path,
        "font_size 11.5\nforeground #c0c6e2\nmap ctrl+c copy_to_clipboard\n",
    )
    .expect("failed to write directive conf");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Directive config"));
    assert_eq!(
        span_color(&preview.lines[0], "font_size"),
        Some(code_palette.function)
    );
    assert_eq!(
        span_color(&preview.lines[1], "#c0c6e2"),
        Some(code_palette.constant)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn ini_style_conf_preview_uses_ini_highlighting() {
    let root = temp_path("ini-conf-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("settings.conf");
    fs::write(&path, "[Settings]\ncolor=blue\nenabled=true\n").expect("failed to write ini conf");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("INI"));
    assert_eq!(
        span_color(&preview.lines[0], "[Settings]"),
        Some(code_palette.r#type)
    );
    assert_eq!(
        span_color(&preview.lines[1], "color"),
        Some(code_palette.parameter)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn shell_style_conf_preview_uses_shell_highlighting() {
    let root = temp_path("shell-conf-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("module.conf");
    fs::write(
        &path,
        "MAKE=\"make -C src/ KERNELDIR=/lib/modules/${kernelver}/build\"\nAUTOINSTALL=yes\n",
    )
    .expect("failed to write shell conf");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Shell"));
    assert_ne!(span_color(&preview.lines[0], "MAKE"), Some(code_palette.fg));
    assert!(line_text(&preview.lines[0]).contains("${kernelver}"));
    assert_ne!(
        span_color(
            &preview.lines[0],
            "\"make -C src/ KERNELDIR=/lib/modules/${kernelver}/build\""
        ),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn config_modeline_can_force_directive_preview() {
    let root = temp_path("kitty-conf-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("settings.conf");
    fs::write(&path, "# vim:ft=kitty\n[Settings]\nforeground #c0c6e2\n")
        .expect("failed to write modeline conf");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Kitty"));
    assert_eq!(
        span_color(&preview.lines[2], "foreground"),
        Some(code_palette.function)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn keys_preview_uses_custom_code_renderer() {
    let root = temp_path("keys");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("bindings.keys");
    fs::write(&path, "ctrl+h=left\nctrl+l=right\n").expect("failed to write keys");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Keys file"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
