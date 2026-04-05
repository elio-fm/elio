mod discovery;
mod input;
mod overlay;
#[cfg(test)]
mod tests;

#[cfg(target_os = "macos")]
use std::path::Path;

#[cfg(target_os = "macos")]
use crate::{
    core::EntryKind,
    file_info::{PreviewKind, inspect_path},
};

#[cfg(target_os = "macos")]
pub(super) fn path_is_text_like(path: &Path) -> bool {
    matches!(
        inspect_path(path, EntryKind::File).preview.kind,
        PreviewKind::Markdown | PreviewKind::Source | PreviewKind::PlainText | PreviewKind::Csv
    )
}
