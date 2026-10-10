use anyhow::{Result, bail};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Save-destination state.  It deliberately owns only chooser semantics: it never
/// creates or renames filesystem entries.
#[derive(Default)]
pub(crate) struct SaveAsState {
    input: String,
    cursor_col: usize,
    open: bool,
    overwrite_path: Option<PathBuf>,
    overwrite_confirmed: bool,
    error: Option<String>,
}

pub(crate) struct SaveAsStartup {
    pub(crate) directory: PathBuf,
    pub(crate) name: String,
}

impl SaveAsState {
    pub(crate) fn new(name: String) -> Self {
        Self {
            cursor_col: name.chars().count(),
            input: name,
            ..Self::default()
        }
    }
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }
    pub(crate) fn open(&mut self) {
        self.open = true;
        self.overwrite_path = None;
        self.overwrite_confirmed = false;
        self.error = None;
    }
    pub(crate) fn close(&mut self) {
        self.open = false;
        self.overwrite_path = None;
        self.overwrite_confirmed = false;
        self.error = None;
    }
    pub(crate) fn input(&self) -> &str {
        &self.input
    }
    pub(crate) fn input_mut(&mut self) -> (&mut String, &mut usize) {
        (&mut self.input, &mut self.cursor_col)
    }
    pub(crate) fn cursor_col(&self) -> usize {
        self.cursor_col
    }
    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub(crate) fn set_error(&mut self, value: impl Into<String>) {
        self.error = Some(value.into());
    }
    pub(crate) fn clear_error(&mut self) {
        self.error = None;
    }
    pub(crate) fn overwrite(&self) -> bool {
        self.overwrite_path.is_some()
    }
    pub(crate) fn overwrite_path(&self) -> Option<&Path> {
        self.overwrite_path.as_deref()
    }
    pub(crate) fn ask_overwrite(&mut self, path: &Path) {
        self.overwrite_path = Some(path.to_path_buf());
        self.overwrite_confirmed = true;
        self.error = None;
    }
    pub(crate) fn overwrite_confirmed(&self) -> bool {
        self.overwrite_confirmed
    }
    pub(crate) fn select_overwrite_confirmation(&mut self, confirmed: bool) {
        self.overwrite_confirmed = confirmed;
    }
    pub(crate) fn toggle_overwrite_confirmation(&mut self) {
        self.overwrite_confirmed = !self.overwrite_confirmed;
    }
    pub(crate) fn cancel_overwrite(&mut self) {
        self.overwrite_path = None;
        self.overwrite_confirmed = false;
    }
}

pub(crate) fn resolve_startup(launch_cwd: &Path, value: Option<&Path>) -> Result<SaveAsStartup> {
    let Some(value) = value.filter(|p| !p.as_os_str().is_empty()) else {
        return Ok(SaveAsStartup {
            directory: launch_cwd.to_path_buf(),
            name: String::new(),
        });
    };
    let path = if value.is_absolute() {
        value.to_path_buf()
    } else {
        launch_cwd.join(value)
    };
    if has_trailing_separator(value) && !path.is_dir() {
        bail!(
            "Cannot save as \"{}\": directory does not exist",
            path.display()
        );
    }
    match fs::metadata(&path) {
        // Directory symlinks are valid places to browse, just as they are in
        // ordinary startup navigation.  A symlink is never a save destination.
        Ok(metadata) if metadata.is_dir() => {
            return Ok(SaveAsStartup {
                directory: path,
                name: String::new(),
            });
        }
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => bail!(
            "Cannot save as \"{}\": unsupported special file",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    if fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        bail!(
            "Cannot save as \"{}\": symlink destinations are not supported",
            path.display()
        );
    }
    parent_and_name_if_dir(&path)
}

fn parent_and_name_if_dir(path: &Path) -> Result<SaveAsStartup> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let meta = fs::metadata(parent).map_err(|_| {
        anyhow::anyhow!(
            "Cannot save as \"{}\": parent directory does not exist",
            path.display()
        )
    })?;
    if !meta.is_dir() {
        bail!(
            "Cannot save as \"{}\": parent is not a directory",
            path.display()
        );
    }
    let name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Cannot save as \"{}\": no file name", path.display()))?
        .to_string_lossy()
        .into_owned();
    Ok(SaveAsStartup {
        directory: parent.to_path_buf(),
        name,
    })
}
fn has_trailing_separator(path: &Path) -> bool {
    path.to_string_lossy().ends_with(std::path::MAIN_SEPARATOR)
        || path.to_string_lossy().ends_with('/')
        || path.to_string_lossy().ends_with('\\')
}

pub(crate) fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        bail!("Enter a valid file name");
    }
    #[cfg(windows)]
    validate_windows_name(name)?;
    Ok(())
}

#[cfg(any(windows, test))]
pub(super) fn validate_windows_name(name: &str) -> Result<()> {
    // Device basenames remain reserved with extensions and regardless of case.
    let basename = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ');
    let basename = basename.to_ascii_uppercase();
    let numbered_device = ["COM", "LPT"].iter().any(|prefix| {
        basename.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    });
    if name
        .chars()
        .any(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
        || name.ends_with(['.', ' '])
        || matches!(
            basename.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        )
        || numbered_device
    {
        bail!("Enter a valid file name");
    }
    Ok(())
}
