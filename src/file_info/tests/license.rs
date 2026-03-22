use super::*;

#[test]
fn license_like_files_detect_specific_and_generic_licenses() {
    let (mit_root, mit_path) = write_temp_file(
        "mit-license",
        "LICENSE",
        "MIT License\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the \"Software\"), to deal\nin the Software without restriction, including without limitation the rights\nto use, copy, modify, merge, publish, distribute, sublicense, and/or sell\ncopies of the Software, and to permit persons to whom the Software is\nfurnished to do so.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND.\n",
    );
    let mit = inspect_path(&mit_path, EntryKind::File);
    assert_eq!(mit.builtin_class, FileClass::License);
    assert_eq!(mit.specific_type_label, Some("MIT License"));
    assert_eq!(mit.preview.kind, PreviewKind::PlainText);
    fs::remove_dir_all(mit_root).expect("failed to remove temp root");

    let (apache_root, apache_path) = write_temp_file(
        "apache-license",
        "LICENSE.md",
        "# SPDX-License-Identifier: Apache-2.0\n\nLicensed under the Apache License, Version 2.0.\n",
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
        "GNU LESSER GENERAL PUBLIC LICENSE\nVersion 2.1, February 1999\n\nThis library is free software; you can redistribute it and/or\nmodify it under the terms of the GNU Lesser General Public\nLicense as published by the Free Software Foundation; either\nversion 2.1 of the License, or (at your option) any later version.\n",
    );
    let copying = inspect_path(&copying_path, EntryKind::File);
    assert_eq!(copying.builtin_class, FileClass::License);
    assert_eq!(copying.specific_type_label, Some("GNU LGPL 2.1 or later"));
    fs::remove_dir_all(copying_root).expect("failed to remove temp root");

    let (hyphen_root, hyphen_path) = write_temp_file(
        "license-prefix",
        "license-mit",
        "MIT License\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the \"Software\"), to deal\nin the Software without restriction, including without limitation the rights\nto use, copy, modify, merge, publish, distribute, sublicense, and/or sell\ncopies of the Software, and to permit persons to whom the Software is\nfurnished to do so.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND.\n",
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
            "CREATIVE COMMONS IST KEINE RECHTSANWALTSKANZLEI UND LEISTET KEINE RECHTSBERATUNG.\n\nLizenz\n\nDER GEGENSTAND DIESER LIZENZ WIRD UNTER DEN BEDINGUNGEN DIESER CREATIVE COMMONS PUBLIC LICENSE ZUR VERFÜGUNG GESTELLT.\n\nSofern zwischen Ihnen und dem Lizenzgeber keine anderweitige Vereinbarung getroffen wurde und soweit Wahlfreiheit besteht, findet auf diesen Lizenzvertrag das Recht der Republik Österreich Anwendung.\n",
            "Creative Commons Attribution 3.0 Austria",
        ),
        (
            "cc-by-sa-at-text",
            "third-party.txt",
            "CREATIVE COMMONS IST KEINE RECHTSANWALTSKANZLEI UND LEISTET KEINE RECHTSBERATUNG.\n\nLizenz\n\nUnter \"Lizenzelementen\" werden im Sinne dieser Lizenz die folgenden übergeordneten Lizenzcharakteristika verstanden: \"Namensnennung\", \"Weitergabe unter gleichen Bedingungen\".\n\nSofern zwischen Ihnen und dem Lizenzgeber keine anderweitige Vereinbarung getroffen wurde und soweit Wahlfreiheit besteht, findet auf diesen Lizenzvertrag das Recht der Republik Österreich Anwendung.\n",
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
            "W3C SOFTWARE NOTICE AND LICENSE\n\nBy obtaining, using and/or copying this work, you (the licensee) agree that you have read, understood, and will comply with the following terms and conditions.\n",
            "W3C Software Notice and License",
        ),
        (
            "wtfpl-text",
            "third-party.txt",
            "DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE\nVersion 2, December 2004\n\nEveryone is permitted to copy and distribute verbatim or modified copies of this license document, and changing it is allowed as long as the name is changed.\n",
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
        "android-shell-wrapper",
        "lld",
        "#!/bin/bash\n#\n# Copyright (C) 2020 The Android Open Source Project\n#\n# Licensed under the Apache License, Version 2.0 (the \"License\");\n# you may not use this file except in compliance with the License.\n# You may obtain a copy of the License at\n#\n#     http://www.apache.org/licenses/LICENSE-2.0\n#\n# Unless required by applicable law or agreed to in writing, software\n# distributed under the License is distributed on an \"AS IS\" BASIS,\n# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.\n\n$(dirname \"$0\")/lld-bin/lld \"$@\"\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("Bash script"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn notice_files_with_embedded_license_text_are_not_classified_as_licenses() {
    let (root, path) = write_temp_file(
        "android-notice",
        "NOTICE.txt",
        "==============================================================================\nAndroid used by:\n  sdk-repo-linux-build-tools.zip\n\nApache License\nVersion 2.0, January 2004\nhttp://www.apache.org/licenses/\n\nTERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Document);
    assert_eq!(facts.specific_type_label, None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
