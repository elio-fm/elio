use super::FileOperationsState;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
enum CopyValue {
    Text(String),
    Items(Vec<PathBuf>),
}

#[derive(Clone, Debug)]
pub(crate) struct CopyOverlayRow {
    pub(crate) shortcut: char,
    pub(crate) label: String,
    pub(crate) status_label: String,
    value: CopyValue,
}

#[derive(Clone, Debug)]
pub(crate) struct CopyOverlay {
    pub(crate) title: String,
    pub(crate) rows: Vec<CopyOverlayRow>,
}

impl FileOperationsState {
    pub(crate) fn copy_items_to_system_clipboard(&mut self, paths: &[PathBuf]) -> String {
        crate::system_clipboard::write_files(paths)
            .map(|()| {
                format!(
                    "Copied {} {}",
                    paths.len(),
                    if paths.len() == 1 { "item" } else { "items" }
                )
            })
            .unwrap_or_else(|error| error.item_status().to_string())
    }
    pub(crate) fn copy_overlay(&self) -> Option<&CopyOverlay> {
        self.copy.as_ref()
    }
    pub(crate) fn dismiss_copy(&mut self) {
        self.copy = None;
    }
    pub(crate) fn open_copy_overlay(&mut self, cwd: &Path, paths: &[PathBuf]) -> bool {
        if paths.is_empty() {
            return false;
        }
        self.copy = Some(build_copy_overlay(cwd, paths));
        true
    }
    pub(crate) fn confirm_copy_index(&mut self, index: usize) -> Option<String> {
        let (value, label) = self
            .copy
            .as_ref()?
            .rows
            .get(index)
            .map(|row| (row.value.clone(), row.status_label.clone()))?;
        let is_items = matches!(value, CopyValue::Items(_));
        let result = match value {
            CopyValue::Text(text) => crate::system_clipboard::write_text(&text),
            CopyValue::Items(paths) => crate::system_clipboard::write_files(&paths),
        };
        Some(match result {
            Ok(()) => {
                self.copy = None;
                format!("Copied {label}")
            }
            Err(error) if is_items => error.item_status().to_string(),
            Err(error) => error.text_status().to_string(),
        })
    }
    pub fn copy_is_open(&self) -> bool {
        self.copy.is_some()
    }
    pub fn copy_title(&self) -> &str {
        self.copy
            .as_ref()
            .map(|copy| copy.title.as_str())
            .unwrap_or("")
    }
    pub fn copy_row_count(&self) -> usize {
        self.copy.as_ref().map(|copy| copy.rows.len()).unwrap_or(0)
    }
    pub fn copy_row_label(&self, index: usize) -> &str {
        self.copy
            .as_ref()
            .and_then(|copy| copy.rows.get(index))
            .map(|row| row.label.as_str())
            .unwrap_or("")
    }
    #[cfg(test)]
    pub fn copy_row_value(&self, index: usize) -> &str {
        self.copy
            .as_ref()
            .and_then(|copy| copy.rows.get(index))
            .and_then(|row| match &row.value {
                CopyValue::Text(text) => Some(text.as_str()),
                CopyValue::Items(_) => None,
            })
            .unwrap_or("")
    }
    pub fn copy_row_shortcut(&self, index: usize) -> Option<char> {
        self.copy
            .as_ref()
            .and_then(|copy| copy.rows.get(index))
            .map(|row| row.shortcut)
    }
}

fn build_copy_overlay(cwd: &Path, paths: &[PathBuf]) -> CopyOverlay {
    let absolute = paths
        .iter()
        .map(|path| {
            if path.is_absolute() {
                path.clone()
            } else {
                cwd.join(path)
            }
        })
        .collect::<Vec<_>>();
    // Recycle Bin entries are named by their original name, not the opaque
    // `$R…` one; their paths stay the real, usable ones.
    let display_names = absolute
        .iter()
        .map(|path| crate::filesystem::recycle_bin_original_name(path).map(PathBuf::from))
        .collect::<Vec<_>>();
    let named = absolute
        .iter()
        .zip(&display_names)
        .map(|(path, name)| name.as_deref().unwrap_or(path))
        .collect::<Vec<_>>();
    let names = named
        .iter()
        .map(|path| {
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    let stems = named
        .iter()
        .map(|path| {
            path.file_stem()
                .unwrap_or_else(|| path.file_name().unwrap_or(path.as_os_str()))
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    let dirs = absolute
        .iter()
        .map(|path| path.parent().unwrap_or(path).display().to_string())
        .collect::<Vec<_>>();
    let plural = absolute.len() != 1;
    let text =
        |shortcut, label: String, status_label: String, values: Vec<String>| CopyOverlayRow {
            shortcut,
            label,
            status_label,
            value: CopyValue::Text(values.join("\n")),
        };
    CopyOverlay {
        title: "Copy to clipboard".to_string(),
        rows: vec![
            text(
                'c',
                if plural {
                    "Copy file names"
                } else {
                    "Copy file name"
                }
                .to_string(),
                if plural { "file names" } else { "file name" }.to_string(),
                names,
            ),
            text(
                'n',
                if plural {
                    "Names without extension"
                } else {
                    "Name without extension"
                }
                .to_string(),
                if plural {
                    "names without extension"
                } else {
                    "name without extension"
                }
                .to_string(),
                stems,
            ),
            text(
                'p',
                if plural { "File paths" } else { "File path" }.to_string(),
                if plural { "file paths" } else { "file path" }.to_string(),
                absolute
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect(),
            ),
            text(
                'd',
                if plural {
                    "Directory paths"
                } else {
                    "Directory path"
                }
                .to_string(),
                if plural {
                    "directory paths"
                } else {
                    "directory path"
                }
                .to_string(),
                dirs,
            ),
            CopyOverlayRow {
                shortcut: 'i',
                label: "Item".to_string(),
                status_label: if plural { "items" } else { "item" }.to_string(),
                value: CopyValue::Items(absolute),
            },
        ],
    }
}
