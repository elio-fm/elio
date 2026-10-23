use super::super::{FileClipboardOperation, freedesktop};
use std::path::PathBuf;

#[test]
fn parses_gnome_copy_payload() {
    let result =
        freedesktop::parse_gnome_for_test(b"copy\nfile:///tmp/a%20b.txt\r\nfile:///tmp/c.txt\r\n")
            .unwrap();
    assert_eq!(result.operation, FileClipboardOperation::Copy);
    assert_eq!(
        result.paths,
        [PathBuf::from("/tmp/a b.txt"), PathBuf::from("/tmp/c.txt")]
    );
}

#[test]
fn parses_gnome_cut_payload() {
    let result = freedesktop::parse_gnome_for_test(b"cut\nfile:///tmp/a.txt").unwrap();
    assert_eq!(result.operation, FileClipboardOperation::Cut);
    assert_eq!(result.paths, [PathBuf::from("/tmp/a.txt")]);
}

#[test]
fn parses_uri_list_and_kde_cut_marker() {
    let result = freedesktop::read_file_clipboard(
        |mime| match mime {
            "text/uri-list" => Some(b"# comment\r\nfile:///tmp/a.txt\r\n".to_vec()),
            "application/x-kde-cutselection" => Some(b"1".to_vec()),
            _ => None,
        },
        &["text/uri-list", "application/x-kde-cutselection"],
    )
    .unwrap();
    assert_eq!(result.operation, FileClipboardOperation::Cut);
}

#[test]
fn parses_kde_uri_list_and_copy_marker() {
    let result = freedesktop::read_file_clipboard(
        |mime| match mime {
            "application/x-kde4-urilist" => Some(b"file:///tmp/a%20b.txt\r\n".to_vec()),
            "application/x-kde-cutselection" => Some(b"0".to_vec()),
            _ => None,
        },
        &[
            "application/x-kde4-urilist",
            "application/x-kde-cutselection",
        ],
    )
    .unwrap();
    assert_eq!(result.operation, FileClipboardOperation::Copy);
    assert_eq!(result.paths, [PathBuf::from("/tmp/a b.txt")]);
}

#[test]
fn ignores_nonlocal_and_malformed_uris() {
    assert!(
        freedesktop::parse_uri_for_test(
            b"trash:///tmp/a.txt\nfile://remote/tmp/b.txt\nfile:///tmp/%GG"
        )
        .is_empty()
    );
}

#[test]
fn publishes_gnome_portable_and_kde_file_representations() {
    let payloads = freedesktop::file_payloads(&[PathBuf::from("/tmp/résumé #1")]);
    assert_eq!(
        payloads
            .iter()
            .map(|(mime, _)| mime.as_str())
            .collect::<Vec<_>>(),
        [
            "x-special/gnome-copied-files",
            "text/uri-list",
            "application/x-kde4-urilist",
            "application/x-kde-cutselection",
            "text/plain;charset=utf-8"
        ]
    );
    assert_eq!(
        String::from_utf8_lossy(&payloads[0].1),
        "copy\nfile:///tmp/r%C3%A9sum%C3%A9%20%231"
    );
    assert_eq!(payloads[1].1, b"file:///tmp/r%C3%A9sum%C3%A9%20%231\r\n");
    assert_eq!(payloads[2].1, payloads[1].1);
    assert_eq!(payloads[3].1, b"0");
}
