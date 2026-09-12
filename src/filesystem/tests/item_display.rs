use super::*;
use std::path::PathBuf;

#[test]
fn size_format_is_human_readable() {
    assert_eq!(format_size(512), "512 B");
    assert_eq!(format_size(2_048), "2.05 kB");
    assert_eq!(format_size(5_488), "5.49 kB");
    assert_eq!(format_size(12_345_678), "12.3 MB");
    assert_eq!(format_size(1_000_000_000_000_000), "1 PB");
    assert_eq!(format_size(u64::MAX), "18.4 EB");
}

#[test]
fn item_count_format_uses_singular_and_grouping() {
    assert_eq!(format_item_count(1), "1 item");
    assert_eq!(format_item_count(24), "24 items");
    assert_eq!(format_item_count(1_234), "1,234 items");
}

#[test]
fn terminal_text_is_sanitized_before_rendering() {
    assert_eq!(
        sanitize_terminal_text("bad\rname\t\u{1b}"),
        "bad^Mname    ^["
    );
}

#[test]
fn symlink_target_label_is_sanitized_before_rendering() {
    let symlink = SymlinkInfo {
        target: Some(PathBuf::from("bad\rname\u{1b}")),
        target_kind: None,
    };

    assert_eq!(symlink_target_display_label(&symlink), "bad^Mname^[");
}

#[test]
fn removes_windows_drive_verbatim_prefix() {
    assert_eq!(
        strip_windows_verbatim_prefix(r"\\?\C:\Users\migue\AppData\Roaming"),
        r"C:\Users\migue\AppData\Roaming"
    );
}

#[test]
fn removes_windows_unc_verbatim_prefix() {
    assert_eq!(
        strip_windows_verbatim_prefix(r"\\?\UNC\server\share\folder"),
        r"\\server\share\folder"
    );
}

#[test]
fn leaves_regular_paths_unchanged() {
    assert_eq!(
        strip_windows_verbatim_prefix("/home/user/project"),
        "/home/user/project"
    );
    assert_eq!(
        strip_windows_verbatim_prefix(r"C:\Users\migue"),
        r"C:\Users\migue"
    );
}
