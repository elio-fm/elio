use super::*;

#[test]
fn code_preview_includes_line_numbers() {
    let root = temp_path("code");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.rs");
    fs::write(&path, "fn main() {}\n").expect("failed to write code");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.lines[0].spans[0].content.contains("1"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn c_preview_uses_code_renderer() {
    let root = temp_path("c");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.c");
    fs::write(
        &path,
        "#include <stdio.h>\nint main(void) {\n    printf(\"hello\\n\");\n}\n",
    )
    .expect("failed to write c source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some_and(|detail| detail.contains('C')));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("printf"))
    );
    assert_ne!(span_color(&preview.lines[0], "#"), Some(code_palette.fg));
    assert_ne!(span_color(&preview.lines[1], "int"), Some(code_palette.fg));
    assert_ne!(
        span_color(&preview.lines[2], "printf"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn python_preview_uses_code_renderer_with_colors() {
    let root = temp_path("python");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.py");
    fs::write(
            &path,
            "@decorator\nclass Greeter:\n    async def greet(self, name: str) -> str:\n        \"\"\"Return greeting.\"\"\"\n        return f\"hi {name}\"\n",
        )
        .expect("failed to write python source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .is_some_and(|detail| detail.contains("Python"))
    );
    assert_ne!(
        span_color(&preview.lines[1], "class"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[1], "Greeter"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[2], "async"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[2], "greet"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[4], "return"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[4], "f\"hi {name}\""),
        Some(code_palette.fg)
    );
    assert!(line_text(&preview.lines[3]).contains("Return greeting."));
    assert!(line_text(&preview.lines[4]).contains("f\"hi {name}\""));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn javascript_preview_uses_code_renderer_with_colors() {
    let root = temp_path("javascript");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.js");
    fs::write(
        &path,
        "export class Greeter {\n  greet(name) { return console.log(`hi ${name}`); }\n}\n",
    )
    .expect("failed to write javascript source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .is_some_and(|detail| detail.contains("JavaScript"))
    );
    assert_ne!(
        span_color(&preview.lines[0], "export"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[0], "Greeter"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[1], "return"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn nix_preview_uses_curated_syntect_support() {
    let root = temp_path("nix");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("flake.nix");
    fs::write(
            &path,
            "{ description = \"elio\"; outputs = { self }: { packages.x86_64-linux.default = self; }; }\n",
        )
        .expect("failed to write nix source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some_and(|detail| detail.contains("Nix")));
    assert_ne!(
        span_color(&preview.lines[0], "description"),
        Some(code_palette.fg)
    );
    assert!(line_has_color(&preview.lines[0], code_palette.string));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("description"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cmake_preview_uses_curated_syntect_support() {
    let root = temp_path("cmake");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("CMakeLists.txt");
    fs::write(
        &path,
        "cmake_minimum_required(VERSION 3.28)\nproject(elio)\nadd_executable(elio main.cpp)\n",
    )
    .expect("failed to write cmake source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .is_some_and(|detail| detail.contains("CMake"))
    );
    assert_ne!(
        span_color(&preview.lines[2], "add_executable"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[1], "project"),
        Some(code_palette.fg)
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("add_executable"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn generic_lockfile_uses_code_renderer() {
    let root = temp_path("lock");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("deps.lock");
    fs::write(&path, "[packages]\nelio=1.0.0\n").expect("failed to write lockfile");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.detail.as_deref(), Some("Lockfile"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("elio"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn makefile_preview_uses_code_renderer() {
    let root = temp_path("makefile");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("Makefile");
    fs::write(
        &path,
        "CC := clang\n.PHONY: build\nbuild: main.o util.o\n\t$(CC) -o app main.o util.o\n",
    )
    .expect("failed to write makefile");

    let preview = build_preview(&file_entry(path.clone()));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Make"))
    );
    assert!(line_texts.iter().any(|text| text.contains(".PHONY")));
    assert!(line_texts.iter().any(|text| text.contains("$(CC)")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn html_preview_uses_code_renderer() {
    let root = temp_path("html");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("index.html");
    fs::write(
        &path,
        "<!DOCTYPE html>\n<div class=\"app\" data-id=\"42\">elio</div>\n",
    )
    .expect("failed to write html");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some());
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("div"))
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("class"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn css_preview_uses_code_renderer() {
    let root = temp_path("css");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("styles.css");
    fs::write(&path, ".app {\n  color: #fff;\n  margin: 12px;\n}\n").expect("failed to write css");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some());
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("color"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn xml_preview_uses_code_renderer() {
    let root = temp_path("xml");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("layout.xml");
    fs::write(&path, "<?xml version=\"1.0\"?>\n<layout id=\"main\" />\n")
        .expect("failed to write xml");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some());
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("layout"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

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

#[test]
fn powershell_preview_uses_curated_syntect_support() {
    let root = temp_path("powershell");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("build.ps1");
    fs::write(
        &path,
        "function Invoke-Greeting([string]$Name) {\n  Write-Host \"Hello $Name\"\n}\n",
    )
    .expect("failed to write powershell script");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("PowerShell"))
    );
    assert_eq!(
        span_color(&preview.lines[0], "function"),
        Some(code_palette.keyword)
    );
    assert_eq!(
        span_color(&preview.lines[0], "Invoke-Greeting"),
        Some(code_palette.function)
    );
    assert_eq!(
        span_color(&preview.lines[0], "[string]"),
        Some(code_palette.r#type)
    );
    assert_eq!(
        span_color(&preview.lines[1], "\"Hello "),
        Some(code_palette.string)
    );
    assert_eq!(
        span_color(&preview.lines[1], "$Name"),
        Some(code_palette.string)
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

#[test]
fn typescript_preview_uses_code_renderer() {
    let root = temp_path("typescript");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.ts");
    fs::write(&path, "export const value: number = 1;\n").expect("failed to write ts");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("TypeScript"))
    );
    assert_ne!(
        span_color(&preview.lines[0], "export"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[0], "const"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[0], "number"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn tsx_preview_uses_code_renderer() {
    let root = temp_path("tsx");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("App.tsx");
    fs::write(
        &path,
        "export function App() { return <div className=\"greeting\">Hello</div>; }\n",
    )
    .expect("failed to write tsx");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("TSX"))
    );
    assert_ne!(
        span_color(&preview.lines[0], "export"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[0], "return"),
        Some(code_palette.fg)
    );
    assert_eq!(span_color(&preview.lines[0], "div"), Some(code_palette.tag));
    assert_eq!(
        span_color(&preview.lines[0], "className"),
        Some(code_palette.parameter)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn curated_syntect_languages_render_with_theme_colors() {
    let root = temp_path("curated-syntect");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let code_palette = theme::code_preview_palette();

    for (name, contents, detail, token) in [
        (
            "schema.sql",
            "SELECT name FROM users WHERE id = 1;\n",
            "SQL",
            "SELECT",
        ),
        (
            "Dockerfile",
            "FROM rust:1.87\nRUN cargo build --release\n",
            "Docker build file",
            "FROM",
        ),
        (
            "main.tf",
            "terraform { required_version = \">= 1.7\" }\n",
            "Terraform module",
            "terraform",
        ),
        (
            "terraform.hcl",
            "server { listen = \"127.0.0.1\" }\n",
            "HCL config",
            "server",
        ),
        (
            "build.gradle",
            "plugins { id 'java' }\n",
            "Gradle build script",
            "'java'",
        ),
        (
            "build.sbt",
            "lazy val root = (project in file(\".\"))\n",
            "sbt build definition",
            "lazy",
        ),
        (
            "script.pl",
            "sub greet { print \"hi\\n\"; }\n",
            "Perl",
            "sub",
        ),
        (
            "Main.hs",
            "module Main where\nmain = putStrLn \"elio\"\n",
            "Haskell",
            "module",
        ),
        (
            "main.jl",
            "function greet(name)\n  return name\nend\n",
            "Julia",
            "function",
        ),
        (
            "analysis.r",
            "library(ggplot2)\nprint(\"elio\")\n",
            "R",
            "library",
        ),
        ("Justfile", "build:\n  cargo test\n", "Just", "build"),
        (
            "styles.scss",
            "$fg: #fff;\n.button { color: $fg; }\n",
            "SCSS",
            "$fg",
        ),
        (
            "theme.sass",
            "$fg: #fff\n.button\n  color: $fg\n",
            "Sass",
            "$fg",
        ),
        (
            "theme.less",
            "@fg: #fff;\n.button { color: @fg; }\n",
            "Less",
            "@fg",
        ),
        (
            "Program.cs",
            "public class Greeter { public string Greet(string name) => name; }\n",
            "C#",
            "public",
        ),
        (
            "main.dart",
            "class Greeter { String greet(String name) => name; }\n",
            "Dart",
            "class",
        ),
        (
            "solver.f90",
            "program elio\n  implicit none\n  print *, \"hello\"\nend program elio\n",
            "Fortran",
            "program",
        ),
        (
            "ledger.cbl",
            "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. ELIOTEST.\n       PROCEDURE DIVISION.\n           DISPLAY \"HELLO\".\n",
            "COBOL",
            "IDENTIFICATION",
        ),
        (
            "main.zig",
            "const std = @import(\"std\");\npub fn main() void {}\n",
            "Zig",
            "@import",
        ),
        (
            "main.kt",
            "class Greeter { fun greet(name: String): String = name }\n",
            "Kotlin",
            "fun",
        ),
        (
            "main.swift",
            "struct Greeter { func greet(name: String) -> String { name } }\n",
            "Swift",
            "func",
        ),
        (
            "main.exs",
            "defmodule Greeter do\n  def greet(name), do: \"hi #{name}\"\nend\n",
            "Elixir",
            "defmodule",
        ),
        (
            "core.clj",
            "(ns elio.core)\n(defn greet [name] (str \"hi \" name))\n",
            "Clojure",
            "defn",
        ),
        (
            "build.ps1",
            "function Invoke-Greeting([string]$Name) {\n  Write-Host \"Hello $Name\"\n}\n",
            "PowerShell",
            "function",
        ),
    ] {
        let path = root.join(name);
        fs::write(&path, contents).expect("failed to write curated syntax fixture");
        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(
            preview
                .detail
                .as_deref()
                .is_some_and(|rendered| rendered.contains(detail)),
            "expected preview detail to mention {detail}"
        );
        assert_ne!(
            span_color(&preview.lines[0], token),
            Some(code_palette.fg),
            "expected {name} to highlight {token}"
        );
    }

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn diff_preview_uses_curated_syntect_support() {
    let root = temp_path("diff");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("changes.diff");
    fs::write(
        &path,
        "diff --git a/src/main.rs b/src/main.rs\nindex 1111111..2222222 100644\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-fn old() {}\n+fn new() {}\n",
    )
    .expect("failed to write diff fixture");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Diff"))
    );
    assert!(
        preview.lines.iter().any(|line| {
            line.spans.iter().any(|span| {
                span.content.trim() != "│"
                    && !span.content.trim().is_empty()
                    && span.style.fg.is_some()
                    && span.style.fg != Some(code_palette.fg)
            })
        }),
        "expected diff preview to contain at least one highlighted token",
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cargo_lock_preview_uses_code_renderer() {
    let root = temp_path("cargo-lock");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("Cargo.lock");
    fs::write(&path, "version = 3\n").expect("failed to write cargo lock");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("TOML"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn code_preview_sanitizes_control_characters() {
    let root = temp_path("control-char-code");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.c");
    let contents = "int main(void) {\n    puts(\"hello \u{1b} world\");\n    return 0;\n}\n";
    fs::write(&path, contents).expect("failed to write control-char source");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    assert!(
        line_texts.iter().any(|line| line.contains("^[ world")),
        "expected control characters to be rendered safely, got: {line_texts:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn code_preview_respects_custom_line_limit() {
    let root = temp_path("code-line-limit");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.rs");
    let text = (1..=12)
        .map(|index| format!("let value_{index} = {index};"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&path, text).expect("failed to write code");

    let preview = build_preview_with_options_and_code_line_limit(
        &file_entry(path),
        &PreviewRequestOptions::Default,
        4,
        &|| false,
    );
    let header = preview
        .header_detail(0, 20)
        .expect("header detail should be present");

    assert_eq!(preview.kind, PreviewKind::Code);
    assert_eq!(preview.lines.len(), 4);
    assert_eq!(
        preview.line_coverage.map(|coverage| coverage.shown_lines),
        Some(4)
    );
    assert!(
        header.contains("showing first 4 lines"),
        "unexpected header: {header}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
