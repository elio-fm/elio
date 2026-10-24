use super::super::{FileClipboardOperation, macos};
use objc2_app_kit::NSPasteboard;
use objc2_foundation::{NSString, NSURL};
use std::{fs, path::PathBuf};

fn pasteboard() -> objc2::rc::Retained<NSPasteboard> {
    macos::unique_pasteboard_for_test()
}

#[test]
fn file_urls_round_trip_multiple_files_directories_and_unicode() {
    let pasteboard = pasteboard();
    let root = std::env::temp_dir().join(format!("elio-system-clipboard-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let paths = [
        root.join("first file.txt"),
        root.join("résumé 📁.txt"),
        root.join("directory with spaces"),
    ];
    fs::write(&paths[0], []).unwrap();
    fs::write(&paths[1], []).unwrap();
    fs::create_dir(&paths[2]).unwrap();

    macos::write_files_for_test(&pasteboard, &paths).unwrap();
    let clipboard = macos::read_files_for_test(&pasteboard).unwrap();
    let expected_text = paths
        .iter()
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(clipboard.paths, paths);
    assert_eq!(clipboard.operation, FileClipboardOperation::Copy);
    assert_eq!(
        macos::read_text_for_test(&pasteboard).as_deref(),
        Some(expected_text.as_str())
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn file_export_includes_native_url_and_text_representations() {
    let pasteboard = pasteboard();
    let paths = [
        PathBuf::from("/tmp/clipboard résumé 📁.txt"),
        PathBuf::from("/tmp/clipboard second file.txt"),
    ];

    macos::write_files_for_test(&pasteboard, &paths).unwrap();

    let items = pasteboard.pasteboardItems().unwrap();
    assert_eq!(items.len(), paths.len());

    for (item, path) in items.iter().zip(&paths) {
        let expected_url = NSURL::from_file_path(path)
            .unwrap()
            .absoluteString()
            .unwrap();
        let expected_path = path.to_string_lossy();

        assert_eq!(
            item.stringForType(&NSString::from_str("public.file-url"))
                .unwrap()
                .to_string(),
            expected_url.to_string()
        );
        assert_eq!(
            item.stringForType(&NSString::from_str("public.utf8-plain-text"))
                .unwrap()
                .to_string(),
            expected_path
        );
        assert_eq!(
            item.stringForType(&NSString::from_str("public.utf16-external-plain-text"))
                .unwrap()
                .to_string(),
            expected_path
        );
    }
}

#[test]
fn imports_only_local_file_urls_as_copy() {
    let pasteboard = pasteboard();
    let file_url = NSURL::from_file_path("/tmp/external file.txt").unwrap();
    let web_url = NSURL::URLWithString(&NSString::from_str("https://example.com/item")).unwrap();
    let remote_file_url =
        NSURL::URLWithString(&NSString::from_str("file://files.example.com/item")).unwrap();
    macos::write_urls_for_test(&pasteboard, vec![file_url, web_url, remote_file_url]).unwrap();

    let clipboard = macos::read_files_for_test(&pasteboard).unwrap();
    assert_eq!(clipboard.paths, [PathBuf::from("/tmp/external file.txt")]);
    assert_eq!(clipboard.operation, FileClipboardOperation::Copy);
}

#[test]
fn ignores_pasteboards_with_only_web_or_remote_file_urls() {
    let pasteboard = pasteboard();
    let web_url = NSURL::URLWithString(&NSString::from_str("https://example.com/item")).unwrap();
    let remote_file_url =
        NSURL::URLWithString(&NSString::from_str("file://files.example.com/item")).unwrap();
    macos::write_urls_for_test(&pasteboard, vec![web_url, remote_file_url]).unwrap();

    assert_eq!(macos::read_files_for_test(&pasteboard), None);
}

#[test]
fn does_not_treat_file_url_text_as_a_native_file_item() {
    let pasteboard = pasteboard();
    macos::write_text_for_test(&pasteboard, "file:///tmp/looks-like-a-file.txt").unwrap();

    assert_eq!(macos::read_files_for_test(&pasteboard), None);
}

#[test]
fn text_round_trips_on_the_same_native_pasteboard() {
    let pasteboard = pasteboard();

    macos::write_text_for_test(&pasteboard, "hello résumé").unwrap();

    assert_eq!(
        macos::read_text_for_test(&pasteboard).as_deref(),
        Some("hello résumé")
    );
}
