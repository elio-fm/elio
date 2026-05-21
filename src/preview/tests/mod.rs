use super::{appearance as theme, *};
#[cfg(unix)]
use crate::core::{EntryKind, SymlinkInfo};
use image::ImageFormat;
use ratatui::{style::Modifier, text::Line};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    fs::File,
    io::Write,
    process::Command,
    sync::{Arc, Barrier},
    thread,
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

mod archives;
mod audio;
mod binaries;
mod code;
mod data;
mod documents;
mod fonts;
mod helpers;
mod images;
mod markdown;
mod structured;
mod text;
mod videos;

use self::helpers::*;

#[test]
fn truncated_directory_preview_omits_sampled_header_count() {
    let root = temp_path("directory-preview-cap");
    let folder = root.join("folder");
    fs::create_dir_all(&folder).expect("failed to create temp folder");
    let line_limit = default_code_preview_line_limit();
    for index in 0..=line_limit {
        fs::write(folder.join(format!("entry-{index:04}.txt")), "")
            .expect("failed to write directory entry");
    }

    let preview = build_preview(&directory_entry(folder.clone()));

    assert_eq!(preview.kind, PreviewKind::Directory);
    assert_eq!(preview.detail, None);
    assert_eq!(preview.lines.len(), line_limit);
    assert_eq!(
        preview.truncation_note.as_deref(),
        Some(format!("{line_limit} items shown").as_str())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn directory_preview_marks_symlink_children_and_targets() {
    use std::{os::unix::fs::symlink, path::PathBuf};

    let root = temp_path("directory-preview-symlinks");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::create_dir_all(root.join("real-dir")).expect("failed to create real directory");
    fs::write(root.join("target.rs"), "fn main() {}\n").expect("failed to write target");

    let dir_target = PathBuf::from("real-dir");
    let file_target = PathBuf::from("target.rs");
    let missing_target = PathBuf::from("../real/code/missing.rs");
    symlink(&dir_target, root.join("linked-dir")).expect("failed to create directory symlink");
    symlink(&file_target, root.join("linked.rs")).expect("failed to create file symlink");
    symlink(&missing_target, root.join("broken.rs")).expect("failed to create broken symlink");

    let preview = build_preview(&directory_entry(root.clone()));

    let linked_dir_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("linked-dir -> real-dir"))
        .expect("directory preview should show symlinked directory target");
    let linked_file_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("linked.rs -> target.rs"))
        .expect("directory preview should show symlinked file target");
    let broken_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("broken.rs -> ../real/code/missing.rs"))
        .expect("directory preview should show broken symlink target");

    let mut linked_dir = directory_entry(root.join("linked-dir"));
    linked_dir.symlink = Some(SymlinkInfo {
        target: Some(dir_target),
        target_kind: Some(EntryKind::Directory),
    });
    let mut linked_file = file_entry(root.join("linked.rs"));
    linked_file.symlink = Some(SymlinkInfo {
        target: Some(file_target),
        target_kind: Some(EntryKind::File),
    });
    let mut broken = file_entry(root.join("broken.rs"));
    broken.symlink = Some(SymlinkInfo {
        target: Some(missing_target),
        target_kind: None,
    });

    assert_eq!(
        linked_dir_line.spans[0].content.as_ref(),
        format!("{} ", theme::resolve_entry(&linked_dir).icon)
    );
    assert_eq!(
        linked_file_line.spans[0].content.as_ref(),
        format!("{} ", theme::resolve_entry(&linked_file).icon)
    );
    assert_eq!(
        broken_line.spans[0].style.fg,
        Some(theme::resolve_entry(&broken).color),
        "broken symlinks should keep the broken-link preview color"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn directory_preview_sanitizes_symlink_names_and_targets() {
    use std::{os::unix::fs::symlink, path::PathBuf};

    let root = temp_path("directory-preview-symlink-sanitized");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let target_label = PathBuf::from("bad\rtarget.txt");
    let target = root.join(&target_label);
    let link_name = "bad\u{1b}link.txt";
    fs::write(&target, "hello").expect("failed to write target");
    symlink(&target_label, root.join(link_name)).expect("failed to create symlink");

    let preview = build_preview(&directory_entry(root.clone()));
    let line_texts = preview.lines.iter().map(line_text).collect::<Vec<_>>();

    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("bad^[link.txt -> bad^Mtarget.txt")),
        "expected directory preview to sanitize symlink names and targets, got: {line_texts:?}"
    );
    assert!(line_texts.iter().all(|line| !line.contains('\r')));
    assert!(line_texts.iter().all(|line| !line.contains('\u{1b}')));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn broken_symlink_preview_reports_target() {
    let root = temp_path("broken-symlink-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let missing = root.join("missing.txt");
    let linked = root.join("linked.txt");
    std::os::unix::fs::symlink(&missing, &linked).expect("failed to create symlink");

    let mut entry = file_entry(linked);
    entry.symlink = Some(SymlinkInfo {
        target: Some(missing.clone()),
        target_kind: None,
    });

    let preview = build_preview(&entry);
    let line_texts = preview.lines.iter().map(line_text).collect::<Vec<_>>();

    assert_eq!(preview.kind, PreviewKind::Unavailable);
    assert_eq!(preview.detail.as_deref(), Some("Broken symlink"));
    assert!(line_texts.iter().any(|line| line == "Broken symbolic link"));
    let missing_label = missing.display().to_string();
    assert!(line_texts.iter().any(|line| line.contains(&missing_label)));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
