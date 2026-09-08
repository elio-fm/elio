use anyhow::Result;
use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Default)]
pub(super) struct Options {
    pub(super) start_dir: Option<PathBuf>,
    pub(super) start_focus: Option<PathBuf>,
    pub(super) reveal_hidden_start_focus: bool,
    pub(super) cwd_file: Option<PathBuf>,
    pub(super) chooser_file: Option<PathBuf>,
    pub(super) config_file: Option<PathBuf>,
    pub(super) theme_file: Option<PathBuf>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ResolvedPath {
    pub(super) directory: PathBuf,
    pub(super) focused_entry: Option<PathBuf>,
    pub(super) reveal_hidden: bool,
}

pub(super) fn resolve_path(arg: &str) -> Result<ResolvedPath> {
    let path = PathBuf::from(arg);
    match fs::metadata(&path) {
        Ok(metadata) if metadata.is_dir() => Ok(ResolvedPath {
            directory: path.canonicalize().unwrap_or(path),
            focused_entry: None,
            reveal_hidden: false,
        }),
        Ok(_) => resolve_file(&path),
        Err(error) => {
            if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
                return resolve_file(&path);
            }
            Err(path_error(&path, &error))
        }
    }
}

fn resolve_file(path: &Path) -> Result<ResolvedPath> {
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Cannot open \"{}\": no file name", path.display()))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let directory = parent
        .canonicalize()
        .map_err(|error| path_error(parent, &error))?;
    Ok(ResolvedPath {
        focused_entry: Some(directory.join(file_name)),
        directory,
        reveal_hidden: should_reveal_hidden(path, file_name),
    })
}

fn should_reveal_hidden(path: &Path, file_name: &OsStr) -> bool {
    file_name.to_string_lossy().starts_with('.') || has_hidden_attribute(path)
}

#[cfg(windows)]
fn has_hidden_attribute(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
}

#[cfg(not(windows))]
fn has_hidden_attribute(_path: &Path) -> bool {
    false
}

fn path_error(path: &Path, error: &io::Error) -> anyhow::Error {
    let detail = match error.kind() {
        io::ErrorKind::NotFound => "no such file or directory".to_string(),
        io::ErrorKind::PermissionDenied => "permission denied".to_string(),
        _ => error.to_string(),
    };
    anyhow::anyhow!("Cannot open \"{}\": {detail}", path.display())
}

#[cfg(test)]
#[path = "tests/options.rs"]
mod tests;
