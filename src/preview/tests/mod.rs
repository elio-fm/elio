use super::{appearance as theme, *};
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
