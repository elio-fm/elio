use super::*;

#[test]
fn extensionless_shebang_scripts_are_classified_as_shell_code() {
    let (root, path) = write_temp_file(
        "extensionless-bash-script",
        "apksigner",
        "#!/bin/bash\n#\n# Copyright (C) 2016 The Android Open Source Project\n#\n# Licensed under the Apache License, Version 2.0 (the \"License\");\n# you may not use this file except in compliance with the License.\n\nexec java -jar apksigner.jar \"$@\"\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("Bash script"));
    assert_eq!(facts.preview.language_hint, Some("bash"));
    assert_code_spec(facts.preview, Some("bash"), CodeBackend::Syntect);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn extensionless_elixir_scripts_are_classified_as_code() {
    let (root, path) = write_temp_file(
        "extensionless-elixir-script",
        "mix-task",
        "#!/usr/bin/env elixir\nIO.puts(\"hello\")\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("Elixir script"));
    assert_eq!(facts.preview.language_hint, Some("elixir"));
    assert_code_spec(facts.preview, Some("elixir"), CodeBackend::Syntect);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn extensionless_powershell_scripts_are_classified_as_code() {
    let (root, path) = write_temp_file(
        "extensionless-powershell-script",
        "elio-tool",
        "#!/usr/bin/env pwsh\nWrite-Host \"hello\"\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("PowerShell script"));
    assert_eq!(facts.preview.language_hint, Some("powershell"));
    assert_code_spec(facts.preview, Some("powershell"), CodeBackend::Syntect);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn extensionless_babashka_scripts_are_classified_as_code() {
    let (root, path) = write_temp_file(
        "extensionless-babashka-script",
        "bb-task",
        "#!/usr/bin/env bb\n(println \"hello\")\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("Clojure script"));
    assert_eq!(facts.preview.language_hint, Some("clojure"));
    assert_code_spec(facts.preview, Some("clojure"), CodeBackend::Syntect);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn ini_style_conf_is_detected_from_contents() {
    let (root, path) = write_temp_file(
        "ini-conf",
        "settings.conf",
        "[Settings]\ncolor=blue\nenabled=true\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_eq!(facts.preview.language_hint, Some("ini"));
    assert_code_spec(
        facts.preview,
        Some("ini"),
        CodeBackend::Custom(CustomCodeKind::Ini),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn shell_style_conf_is_detected_from_contents() {
    let (root, path) = write_temp_file(
        "shell-conf",
        "module.conf",
        "MAKE=\"make -C src/ KERNELDIR=/lib/modules/${kernelver}/build\"\nAUTOINSTALL=yes\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_eq!(facts.preview.language_hint, Some("sh"));
    assert_code_spec(facts.preview, Some("sh"), CodeBackend::Syntect);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn ambiguous_conf_defaults_to_directive_config() {
    let (root, path) = write_temp_file(
        "directive-conf",
        "custom.conf",
        "font_size 11.5\nforeground #c0c6e2\nmap ctrl+c copy_to_clipboard\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_eq!(facts.preview.language_hint, Some("config"));
    assert_code_spec(
        facts.preview,
        Some("config"),
        CodeBackend::Custom(CustomCodeKind::DirectiveConf),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cfg_files_use_the_same_content_based_detection() {
    let (root, path) = write_temp_file(
        "directive-cfg",
        "custom.cfg",
        "font_size 11.5\nforeground #c0c6e2\nmap ctrl+c copy_to_clipboard\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_code_spec(
        facts.preview,
        Some("config"),
        CodeBackend::Custom(CustomCodeKind::DirectiveConf),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn config_modelines_can_force_directive_conf_without_name_overrides() {
    let (root, path) = write_temp_file(
        "kitty-modeline",
        "settings.conf",
        "# vim:ft=kitty\n[Settings]\ncolor=blue\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_eq!(facts.preview.language_hint, Some("kitty"));
    assert_code_spec(
        facts.preview,
        Some("kitty"),
        CodeBackend::Custom(CustomCodeKind::DirectiveConf),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn unsupported_modelines_are_ignored_for_conf_detection() {
    let (root, path) = write_temp_file(
        "unknown-modeline",
        "settings.conf",
        "# vim:ft=totallyunknown\n[Settings]\ncolor=blue\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_eq!(facts.preview.language_hint, Some("ini"));
    assert_code_spec(
        facts.preview,
        Some("ini"),
        CodeBackend::Custom(CustomCodeKind::Ini),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn extensionless_png_is_detected_from_magic_bytes() {
    let root = temp_path("extensionless-png");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("background");
    fs::write(
        &path,
        [
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H',
            b'D', b'R',
        ],
    )
    .expect("failed to write png signature");

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Image);
    assert_eq!(facts.specific_type_label, Some("PNG image"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn extensionless_svg_is_detected_from_contents() {
    let root = temp_path("extensionless-svg");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("logo");
    fs::write(
        &path,
        r#"<?xml version="1.0"?><svg viewBox="0 0 600 300" xmlns="http://www.w3.org/2000/svg"></svg>"#,
    )
    .expect("failed to write svg contents");

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Image);
    assert_eq!(facts.specific_type_label, Some("SVG image"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
