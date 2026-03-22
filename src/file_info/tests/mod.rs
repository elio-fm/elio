use super::*;
use crate::app::{EntryKind, FileClass};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-file-info-{label}-{unique}"))
}

fn write_temp_file(label: &str, file_name: &str, contents: &str) -> (PathBuf, PathBuf) {
    let root = temp_path(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join(file_name);
    fs::write(&path, contents).expect("failed to write temp file");
    (root, path)
}

fn assert_code_spec(
    preview: PreviewSpec,
    code_syntax: Option<&'static str>,
    code_backend: CodeBackend,
) {
    assert_eq!(preview.code_syntax, code_syntax);
    assert_eq!(preview.code_backend, code_backend);
}

mod classify;
mod extensions;
mod license;
mod names;
