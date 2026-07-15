use super::*;
use crate::core::{Entry, EntryKind, FileClass};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::{ffi::CString, os::unix::ffi::OsStrExt};

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

#[cfg(unix)]
fn make_fifo(path: &Path) {
    let c_path =
        CString::new(path.as_os_str().as_bytes()).expect("fifo path should not contain NUL");
    let result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o644) };
    assert_eq!(
        result,
        0,
        "failed to create fifo at {}: {}",
        path.display(),
        std::io::Error::last_os_error()
    );
}

mod classify;
mod extensions;
mod license;
mod names;
