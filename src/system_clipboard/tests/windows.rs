use super::super::{FileClipboardOperation, windows};
use std::path::PathBuf;
use windows_sys::Win32::System::Ole::{DROPEFFECT_COPY, DROPEFFECT_MOVE};

#[test]
fn file_drop_payload_round_trips_multiple_unicode_paths() {
    let paths = [
        PathBuf::from(r"C:\Users\elio\first file.txt"),
        PathBuf::from(r"C:\Users\elio\résumé 📁"),
        PathBuf::from(r"C:\Users\elio\directory with spaces"),
    ];
    let bytes = windows::file_drop_data_for_test(&paths).unwrap();

    assert_eq!(
        windows::drop_files_from_bytes_for_test(&bytes).unwrap(),
        paths
    );
}

#[test]
fn preferred_drop_effect_preserves_windows_copy_and_cut_semantics() {
    assert_eq!(
        windows::operation_from_drop_effect_for_test(DROPEFFECT_COPY),
        FileClipboardOperation::Copy
    );
    assert_eq!(
        windows::operation_from_drop_effect_for_test(DROPEFFECT_MOVE),
        FileClipboardOperation::Cut
    );
}

#[test]
fn unicode_text_payload_decodes_until_the_nul_terminator() {
    let bytes = "hello résumé\0ignored"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();

    assert_eq!(
        windows::text_from_utf16_bytes_for_test(&bytes).as_deref(),
        Some("hello résumé")
    );
}

#[test]
fn file_text_strips_verbatim_prefixes_without_changing_file_drop_paths() {
    let paths = [
        PathBuf::from(r"\\?\C:\Users\miguel\Documents\PowerShell"),
        PathBuf::from(r"\\?\UNC\server\share\folder with spaces"),
    ];

    assert_eq!(
        windows::file_text_for_test(&paths).as_deref(),
        Some("C:\\Users\\miguel\\Documents\\PowerShell\r\n\\\\server\\share\\folder with spaces")
    );

    let drop_data = windows::file_drop_data_for_test(&paths).unwrap();
    assert_eq!(
        windows::drop_files_from_bytes_for_test(&drop_data).unwrap(),
        paths,
        "CF_HDROP must retain verbatim paths"
    );
}
