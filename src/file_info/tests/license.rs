use super::*;

#[test]
fn license_like_files_detect_specific_and_generic_licenses() {
    let (mit_root, mit_path) = write_temp_file(
        "mit-license",
        "LICENSE",
        "SPDX-License-Identifier: MIT\n\nFixture grant notes.\n",
    );
    let mit = inspect_path(&mit_path, EntryKind::File);
    assert_eq!(mit.builtin_class, FileClass::License);
    assert_eq!(mit.specific_type_label, Some("MIT License"));
    assert_eq!(mit.preview.kind, PreviewKind::PlainText);
    fs::remove_dir_all(mit_root).expect("failed to remove temp root");

    let (apache_root, apache_path) = write_temp_file(
        "apache-license",
        "LICENSE.md",
        "# SPDX-License-Identifier: Apache-2.0\n\nFixture license notes.\n",
    );
    let apache = inspect_path(&apache_path, EntryKind::File);
    assert_eq!(apache.builtin_class, FileClass::License);
    assert_eq!(apache.specific_type_label, Some("Apache License 2.0"));
    assert_eq!(apache.preview.kind, PreviewKind::Markdown);
    fs::remove_dir_all(apache_root).expect("failed to remove temp root");

    let (generic_root, generic_path) = write_temp_file(
        "generic-license",
        "LICENSE.txt",
        "Copyright (c) 2026 Example Corp.\nAll rights reserved.\nThis license governs internal use only.\nNo warranty is provided.\n",
    );
    let generic = inspect_path(&generic_path, EntryKind::File);
    assert_eq!(generic.builtin_class, FileClass::License);
    assert_eq!(generic.specific_type_label, Some("License document"));
    fs::remove_dir_all(generic_root).expect("failed to remove temp root");

    let (copying_root, copying_path) = write_temp_file(
        "copying-lesser",
        "COPYING.LESSER",
        "GNU LESSER GENERAL PUBLIC LICENSE\nVersion 2.1\nor any later version\n",
    );
    let copying = inspect_path(&copying_path, EntryKind::File);
    assert_eq!(copying.builtin_class, FileClass::License);
    assert_eq!(copying.specific_type_label, Some("GNU LGPL 2.1 or later"));
    fs::remove_dir_all(copying_root).expect("failed to remove temp root");

    let (hyphen_root, hyphen_path) = write_temp_file(
        "license-prefix",
        "license-mit",
        "SPDX-License-Identifier: MIT\n\nFixture grant notes.\n",
    );
    let hyphen = inspect_path(&hyphen_path, EntryKind::File);
    assert_eq!(hyphen.builtin_class, FileClass::License);
    assert_eq!(hyphen.specific_type_label, Some("MIT License"));
    fs::remove_dir_all(hyphen_root).expect("failed to remove temp root");
}

#[test]
fn license_detection_requires_real_markers_not_just_a_filename() {
    let (root, path) = write_temp_file(
        "not-a-license",
        "LICENSE",
        "shopping list\n- apples\n- oranges\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::File);
    assert_eq!(facts.specific_type_label, None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn license_family_matching_rejects_middle_substrings_without_markers() {
    let (root, path) = write_temp_file(
        "license-middle-substring",
        "my-license-notes.txt",
        "notes about legal cleanup\nnot a real license file\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Document);
    assert_eq!(facts.specific_type_label, None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn spdx_marked_text_files_can_be_detected_without_license_filenames() {
    let (root, path) = write_temp_file(
        "spdx-text",
        "third-party.txt",
        "SPDX-License-Identifier: MIT\n\nRedistribution notes.\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::License);
    assert_eq!(facts.specific_type_label, Some("MIT License"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn additional_spdx_license_ids_are_classified_explicitly() {
    let cases = [
        ("cc-by-3", "CC-BY-3.0", "Creative Commons Attribution 3.0"),
        (
            "cc-by-at",
            "CC-BY-3.0-AT",
            "Creative Commons Attribution 3.0 Austria",
        ),
        (
            "cc-by-sa-jp",
            "CC-BY-SA-2.1-JP",
            "Creative Commons Attribution-ShareAlike 2.1 Japan",
        ),
        (
            "cc-by-sa-at",
            "CC-BY-SA-3.0-AT",
            "Creative Commons Attribution-ShareAlike 3.0 Austria",
        ),
        ("w3c", "W3C", "W3C Software Notice and License"),
        ("wtfpl", "WTFPL", "WTFPL"),
    ];

    for (label, spdx_id, expected) in cases {
        let (root, path) = write_temp_file(
            label,
            "LICENSE",
            &format!("SPDX-License-Identifier: {spdx_id}\n\nLicense text.\n"),
        );

        let facts = inspect_path(&path, EntryKind::File);

        assert_eq!(facts.builtin_class, FileClass::License);
        assert_eq!(facts.specific_type_label, Some(expected));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn high_signal_license_texts_are_detected_without_canonical_filenames() {
    let cases = [
        (
            "cc-by-at-text",
            "third-party.txt",
            "CREATIVE COMMONS IST KEINE RECHTSANWALTSKANZLEI UND LEISTET KEINE RECHTSBERATUNG.\nCREATIVE COMMONS PUBLIC LICENSE.\nRECHT DER REPUBLIK ÖSTERREICH ANWENDUNG.\n",
            "Creative Commons Attribution 3.0 Austria",
        ),
        (
            "cc-by-sa-at-text",
            "third-party.txt",
            "CREATIVE COMMONS IST KEINE RECHTSANWALTSKANZLEI UND LEISTET KEINE RECHTSBERATUNG.\nWEITERGABE UNTER GLEICHEN BEDINGUNGEN.\nRECHT DER REPUBLIK ÖSTERREICH ANWENDUNG.\n",
            "Creative Commons Attribution-ShareAlike 3.0 Austria",
        ),
        (
            "cc-by-sa-jp-text",
            "third-party.txt",
            "アトリビューション—シェアアライク 2.1\n（帰属—同一条件許諾）\n利用許諾\n",
            "Creative Commons Attribution-ShareAlike 2.1 Japan",
        ),
        (
            "w3c-text",
            "third-party.txt",
            "W3C SOFTWARE NOTICE AND LICENSE\nBy obtaining, using and/or copying this work.\n",
            "W3C Software Notice and License",
        ),
        (
            "wtfpl-text",
            "third-party.txt",
            "DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE\nEveryone is permitted to copy and distribute verbatim or modified copies.\n",
            "WTFPL",
        ),
    ];

    for (label, file_name, contents, expected) in cases {
        let (root, path) = write_temp_file(label, file_name, contents);

        let facts = inspect_path(&path, EntryKind::File);

        assert_eq!(facts.builtin_class, FileClass::License);
        assert_eq!(facts.specific_type_label, Some(expected));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn standalone_apache_license_text_is_detected_without_canonical_filename() {
    let (root, path) = write_temp_file(
        "apache-third-party",
        "third-party.txt",
        "Apache License\nVersion 2.0, January 2004\nhttp://www.apache.org/licenses/LICENSE-2.0\n\nTERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::License);
    assert_eq!(facts.specific_type_label, Some("Apache License 2.0"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn phase_numbers_do_not_trigger_japanese_cc_license_detection() {
    let (root, path) = write_temp_file(
        "roadmap-phase-numbers",
        "RoadMap2026.txt",
        "Phase 2: The \"Modern Systems\" Language (Month 2)\n\nGetting Started with Rust (LFEL1002) [1.5h]: A quick syntax primer.\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Document);
    assert_eq!(facts.specific_type_label, None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn diff_like_numeric_text_does_not_trigger_japanese_cc_license_detection() {
    let (root, path) = write_temp_file(
        "diff-like-numbers",
        "undo this.txt",
        "undo this\n\n145 app frame state preview content area some rect\n146 x 2\n147 y 3\n148 width 48\n149 height 20\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Document);
    assert_eq!(facts.specific_type_label, None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn embedded_license_headers_do_not_turn_shell_wrappers_into_license_files() {
    let (root, path) = write_temp_file(
        "shell-wrapper",
        "tool",
        "#!/bin/bash\n#\n# Copyright (C) 2026 Example Project\n# SPDX-License-Identifier: Apache-2.0\n\n$(dirname \"$0\")/fixture-bin/tool \"$@\"\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("Bash script"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn notice_files_with_embedded_license_text_are_not_classified_as_licenses() {
    let (root, path) = write_temp_file(
        "notice-bundle",
        "NOTICE.txt",
        "==============================================================================\nExample component used by:\n  fixture-package.zip\n\nApache License\nVersion 2.0, January 2004\nhttp://www.apache.org/licenses/\n\nTERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Document);
    assert_eq!(facts.specific_type_label, None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
