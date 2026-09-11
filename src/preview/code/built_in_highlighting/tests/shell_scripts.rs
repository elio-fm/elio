use super::*;

#[test]
fn pkgbuild_preview_uses_shell_renderer() {
    let root = temp_path("pkgbuild");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("PKGBUILD");
    fs::write(
        &path,
        "pkgname=elio\nbuild() {\n  cargo build --release\n}\n",
    )
    .expect("failed to write pkgbuild");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn shell_script_preview_uses_code_renderer() {
    let root = temp_path("shell");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("deploy.sh");
    fs::write(
            &path,
            "#!/usr/bin/env bash\nNAME=elio\nif [ -n \"$NAME\" ]; then\n  printf '%s\\n' \"$(whoami)\"\nfi\n",
        )
        .expect("failed to write shell script");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Shell"))
    );
    assert!(line_texts.iter().any(|text| text.contains("printf")));
    assert!(line_texts.iter().any(|text| text.contains("$(whoami)")));
    assert_ne!(span_color(&preview.lines[2], "if"), Some(code_palette.fg));
    assert_ne!(
        span_color(&preview.lines[3], "printf"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[3], "$(whoami)"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sh_file_preview_keeps_core_shell_tokens_non_gray() {
    let root = temp_path("sh-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("deploy.sh");
    fs::write(
        &path,
        "NAME=elio\nif [ -n \"$HOME\" ]; then\n  echo \"$NAME\"\nfi # done\n",
    )
    .expect("failed to write sh script");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Shell"))
    );
    assert_ne!(span_color(&preview.lines[0], "NAME"), Some(code_palette.fg));
    assert_ne!(span_color(&preview.lines[1], "if"), Some(code_palette.fg));
    assert_ne!(span_color(&preview.lines[1], "$"), Some(code_palette.fg));
    assert_ne!(span_color(&preview.lines[1], "HOME"), Some(code_palette.fg));
    assert_ne!(span_color(&preview.lines[2], "echo"), Some(code_palette.fg));
    assert_eq!(
        span_color(&preview.lines[3], "#"),
        Some(code_palette.comment)
    );
    assert_eq!(
        span_color(&preview.lines[3], " done"),
        Some(code_palette.comment)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sh_preview_highlights_plain_commands_and_options() {
    let root = temp_path("sh-commands-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("deploy.sh");
    fs::write(
        &path,
        "deploy() {\n  grep -q \"$HOME\" /etc/profile\n  my_tool --flag \"$NAME\"\n}\n",
    )
    .expect("failed to write sh commands script");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_ne!(
        span_color(&preview.lines[0], "deploy"),
        Some(code_palette.fg)
    );
    assert_ne!(span_color(&preview.lines[1], "grep"), Some(code_palette.fg));
    assert_ne!(span_color(&preview.lines[1], "-q"), Some(code_palette.fg));
    assert_ne!(
        span_color(&preview.lines[2], "my_tool"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[2], "--flag"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sh_preview_highlights_common_builtins_and_redirections() {
    let root = temp_path("sh-builtins-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("deploy.sh");
    fs::write(
        &path,
        "#!/bin/sh\nset -e\ncd /tmp\ntrap 'cleanup' EXIT\nexport PATH=\"$HOME/bin:$PATH\"\nsource ./env.sh\nread -r NAME\nexec \"$NAME\" > /tmp/out.log\n",
    )
    .expect("failed to write sh builtins script");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(
        span_color(&preview.lines[0], "#!"),
        Some(code_palette.r#macro)
    );
    assert_ne!(span_color(&preview.lines[1], "set"), Some(code_palette.fg));
    assert_ne!(span_color(&preview.lines[2], "cd"), Some(code_palette.fg));
    assert_ne!(span_color(&preview.lines[3], "trap"), Some(code_palette.fg));
    assert_ne!(
        span_color(&preview.lines[4], "export"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[5], "source"),
        Some(code_palette.fg)
    );
    assert_ne!(span_color(&preview.lines[6], "read"), Some(code_palette.fg));
    assert_ne!(span_color(&preview.lines[7], "exec"), Some(code_palette.fg));
    assert_ne!(span_color(&preview.lines[7], ">"), Some(code_palette.fg));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn shell_dotfile_preview_uses_code_renderer() {
    let root = temp_path("shell-dotfile");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join(".bashrc");
    fs::write(
        &path,
        "export PATH=\"$HOME/bin:$PATH\"\nalias ll='ls -la'\n",
    )
    .expect("failed to write shell dotfile");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Bash"))
    );
    assert!(line_texts.iter().any(|text| text.contains("export")));
    assert!(line_texts.iter().any(|text| text.contains("alias")));
    assert_ne!(
        span_color(&preview.lines[0], "export"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[1], "alias"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn zsh_preview_uses_shell_specific_support() {
    let root = temp_path("zsh");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("prompt.zsh");
    fs::write(
        &path,
        "autoload -U colors && colors\nprompt_elio() {\n  print -P '%F{blue}%~%f'\n}\n",
    )
    .expect("failed to write zsh script");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some());
    assert!(line_texts.iter().any(|text| text.contains("autoload")));
    assert!(line_texts.iter().any(|text| text.contains("prompt_elio")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
