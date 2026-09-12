use anyhow::Result;
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

pub(crate) fn write_selected_paths(chooser_file: Option<&Path>, paths: &[PathBuf]) -> Result<()> {
    let Some(chooser_file) = chooser_file else {
        return Ok(());
    };

    let bytes = output_bytes(paths);
    if file_is_stdout(chooser_file) {
        let mut stdout = io::stdout().lock();
        stdout.write_all(&bytes)?;
        stdout.flush()?;
    } else {
        std::fs::write(chooser_file, bytes)?;
    }
    Ok(())
}

pub(super) fn file_is_stdout(chooser_file: &Path) -> bool {
    chooser_file == Path::new("-") || file_is_dev_stdout(chooser_file)
}

#[cfg(unix)]
fn file_is_dev_stdout(chooser_file: &Path) -> bool {
    chooser_file == Path::new("/dev/stdout")
}

#[cfg(not(unix))]
fn file_is_dev_stdout(_chooser_file: &Path) -> bool {
    false
}

#[cfg(unix)]
fn output_bytes(paths: &[PathBuf]) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    let mut bytes = Vec::new();
    for path in paths {
        bytes.extend_from_slice(path.as_os_str().as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

#[cfg(not(unix))]
fn output_bytes(paths: &[PathBuf]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for path in paths {
        bytes.extend_from_slice(path.to_string_lossy().as_bytes());
        bytes.push(b'\n');
    }
    bytes
}
