use super::FileOperationsState;
use std::{fs, path::Path, path::PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct RenameOverlay {
    pub(crate) is_dir: bool,
    pub(crate) original_name: String,
    pub(crate) input: String,
    pub(crate) cursor_col: usize,
    pub(crate) error: Option<String>,
}

pub(crate) struct RenameCompletion {
    pub(crate) new_path: PathBuf,
    pub(crate) status: String,
}

impl FileOperationsState {
    pub(crate) fn open_rename_prompt(&mut self, name: String, is_dir: bool) {
        let cursor_col = cursor_before_extension(&name);
        self.create = None;
        self.trash = None;
        self.restore = None;
        self.rename = Some(RenameOverlay {
            is_dir,
            original_name: name.clone(),
            input: name,
            cursor_col,
            error: None,
        });
    }

    pub(crate) fn prepare_rename_names(&mut self) -> Option<(String, String)> {
        let rename = self.rename.as_ref()?;
        let original_name = rename.original_name.clone();
        let new_name = rename.input.trim().to_string();
        if new_name.is_empty() {
            if let Some(rename) = &mut self.rename {
                rename.error = Some("Name cannot be empty".to_string());
            }
            return None;
        }
        if new_name.contains('/') {
            if let Some(rename) = &mut self.rename {
                rename.error = Some("Name cannot contain /".to_string());
            }
            return None;
        }
        if new_name == original_name {
            self.rename = None;
            return None;
        }
        Some((original_name, new_name))
    }

    pub(crate) fn confirm_rename(
        &mut self,
        cwd: &Path,
        old_path: Option<PathBuf>,
    ) -> Option<RenameCompletion> {
        let Some(r) = &self.rename else {
            return None;
        };
        let new_name = r.input.trim().to_string();
        let original_name = r.original_name.clone();

        if new_name.is_empty() {
            if let Some(r) = &mut self.rename {
                r.error = Some("Name cannot be empty".to_string());
            }
            return None;
        }
        if new_name.contains('/') {
            if let Some(r) = &mut self.rename {
                r.error = Some("Name cannot contain /".to_string());
            }
            return None;
        }
        if new_name == original_name {
            self.rename = None;
            return None;
        }
        let new_path = cwd.join(&new_name);
        if new_path.exists() {
            if let Some(r) = &mut self.rename {
                r.error = Some(format!("\"{}\" already exists", new_name));
            }
            return None;
        }

        let Some(old_path) = old_path else {
            self.rename = None;
            return None;
        };
        if let Err(error) = fs::rename(&old_path, &new_path) {
            let msg = match error.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    format!("Permission denied renaming \"{}\"", original_name)
                }
                _ => format!("Could not rename: {error}"),
            };
            if let Some(r) = &mut self.rename {
                r.error = Some(msg);
            }
            return None;
        }

        self.rename = None;
        let status = format!("Renamed \"{}\" → \"{}\"", original_name, new_name);
        Some(RenameCompletion { new_path, status })
    }
}

pub(super) fn cursor_before_extension(name: &str) -> usize {
    let total = name.chars().count();
    if let Some(dot_pos) = name.rfind('.') {
        let dot_char = name[..dot_pos].chars().count();
        if dot_char > 0 {
            return dot_char;
        }
    }
    total
}

impl FileOperationsState {
    pub fn rename_is_open(&self) -> bool {
        self.rename.is_some()
    }

    pub fn rename_input(&self) -> &str {
        self.rename.as_ref().map_or("", |r| &r.input)
    }

    pub fn rename_cursor_col(&self) -> usize {
        self.rename.as_ref().map_or(0, |r| r.cursor_col)
    }

    pub fn rename_original_name(&self) -> &str {
        self.rename.as_ref().map_or("", |r| &r.original_name)
    }

    pub fn rename_item_is_dir(&self) -> bool {
        self.rename.as_ref().is_some_and(|r| r.is_dir)
    }

    pub fn rename_error(&self) -> Option<&str> {
        self.rename.as_ref().and_then(|r| r.error.as_deref())
    }
}
