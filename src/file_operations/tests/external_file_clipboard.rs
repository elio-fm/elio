use crate::file_operations::{ClipOp, external_file_clipboard::*};
use std::path::PathBuf;

#[test]
fn parses_gnome_copy_payload() {
    let result = parse_gnome_copied_files(b"copy\nfile:///tmp/a%20b.txt\r\nfile:///tmp/c.txt\r\n")
        .expect("GNOME file clipboard should parse");

    assert_eq!(result.op, ClipOp::Yank);
    assert_eq!(
        result.paths,
        [PathBuf::from("/tmp/a b.txt"), PathBuf::from("/tmp/c.txt")]
    );
}

#[test]
fn parses_gnome_cut_payload() {
    let result = parse_gnome_copied_files(b"cut\nfile:///tmp/a.txt")
        .expect("GNOME cut clipboard should parse");

    assert_eq!(result.op, ClipOp::Cut);
    assert_eq!(result.paths, [PathBuf::from("/tmp/a.txt")]);
}

#[test]
fn parses_uri_list_and_kde_cut_marker() {
    let mut read = |mime: &str| match mime {
        "text/uri-list" => Some(b"# comment\r\nfile:///tmp/a.txt\r\n".to_vec()),
        "application/x-kde-cutselection" => Some(b"1".to_vec()),
        _ => None,
    };
    let result = read_clipboard_types(
        &mut read,
        &["text/uri-list", "application/x-kde-cutselection"],
    )
    .expect("KDE file clipboard should parse");

    assert_eq!(result.op, ClipOp::Cut);
    assert_eq!(result.paths, [PathBuf::from("/tmp/a.txt")]);
}

#[test]
fn ignores_nonlocal_uri_schemes_and_malformed_file_uris() {
    assert!(
        parse_uri_list(b"trash:///tmp/a.txt\nfile://remote/tmp/b.txt\nfile:///tmp/%GG").is_empty()
    );
}

#[test]
fn requires_a_file_clipboard_mime_type() {
    let result = read_clipboard_types(|_| None, &["text/plain"]);
    assert!(result.is_none());
}
