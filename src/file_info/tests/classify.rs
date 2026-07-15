use super::*;

#[test]
fn extensionless_shebang_scripts_are_classified_as_shell_code() {
    let (root, path) = write_temp_file(
        "extensionless-bash-script",
        "tool-wrapper",
        "#!/bin/bash\n#\n# Copyright (C) 2026 Example Project\n# SPDX-License-Identifier: Apache-2.0\n\nexec java -jar fixture.jar \"$@\"\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("Bash script"));
    assert_eq!(facts.preview.language_hint, Some("bash"));
    assert_code_spec(facts.preview, Some("bash"), CodeBackend::Syntect);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn fast_extensionless_shebang_scripts_are_not_mislabeled_as_licenses() {
    let (root, path) = write_temp_file(
        "fast-extensionless-shell-script",
        "configure",
        "#!/bin/sh\n\n# SPDX-License-Identifier: ISC\n#\n# configure - POSIX shell build configuration framework\n\nset -e\n",
    );
    let metadata = fs::metadata(&path).expect("metadata should exist");
    let entry = Entry {
        path: path.clone(),
        name: "configure".to_string(),
        name_key: "configure".to_string(),
        kind: EntryKind::File,
        symlink: None,
        size: metadata.len(),
        modified: metadata.modified().ok(),
        readonly: false,
    };

    let facts = inspect_entry_fast(&entry);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("Shell script"));
    assert_eq!(facts.preview.language_hint, Some("sh"));
    assert_code_spec(facts.preview, Some("sh"), CodeBackend::Syntect);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn input_templates_with_shebangs_use_script_language() {
    let (root, path) = write_temp_file(
        "bash-template",
        "_pkg.bash.in",
        "#! /usr/bin/env bash\n\n_pkg_complete() {\n    :\n}\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("Bash template"));
    assert_eq!(facts.preview.language_hint, Some("bash"));
    assert_code_spec(facts.preview, Some("bash"), CodeBackend::Syntect);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn zsh_completion_input_templates_are_detected_from_compdef() {
    let (root, path) = write_temp_file(
        "zsh-completion-template",
        "_pkg.in",
        "#compdef pkg pkg-static\n\n_pkg_cmd() {\n    %prefix%/sbin/pkg \"$@\"\n}\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("Zsh completion template"));
    assert_eq!(facts.preview.language_hint, Some("zsh"));
    assert_code_spec(facts.preview, Some("zsh"), CodeBackend::Syntect);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn unknown_input_fixtures_stay_plain_files() {
    let (root, path) = write_temp_file(
        "unknown-input-fixture",
        "1.in",
        "{\n\"key1\": value;\n\"key1\": value2;\n}\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::File);
    assert_eq!(facts.specific_type_label, None);
    assert_eq!(facts.preview, PreviewSpec::plain_text());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn binary_named_input_templates_stay_plain_files() {
    let facts = inspect_path(Path::new("logo.png.in"), EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::File);
    assert_eq!(facts.specific_type_label, None);
    assert_eq!(facts.preview, PreviewSpec::plain_text());
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
fn entry_display_name_controls_classification_when_storage_name_has_collision_suffix() {
    let root = temp_path("display-name-classification");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("photo.jpeg.2");
    fs::write(&path, [0xff, 0xd8, 0xff, 0xdb]).expect("failed to write jpeg signature");
    let metadata = fs::metadata(&path).expect("failed to stat temp file");
    let entry = crate::core::Entry {
        path: path.clone(),
        name: "photo.jpeg".to_string(),
        name_key: "photo.jpeg".to_string(),
        kind: EntryKind::File,
        symlink: None,
        size: metadata.len(),
        modified: metadata.modified().ok(),
        readonly: false,
    };

    let facts = inspect_entry_cached(&entry);

    assert_eq!(facts.builtin_class, FileClass::Image);
    assert_eq!(facts.specific_type_label, Some("JPEG image"));

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

#[cfg(unix)]
#[test]
fn extensionless_fifo_is_not_opened_for_content_sniffing() {
    let root = temp_path("extensionless-fifo");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("hidraw-like");
    make_fifo(&path);

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::File);
    assert_eq!(facts.specific_type_label, None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn config_fifo_is_not_opened_for_content_sniffing() {
    let root = temp_path("config-fifo");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("settings.conf");
    make_fifo(&path);

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_eq!(facts.specific_type_label, Some("Config file"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
