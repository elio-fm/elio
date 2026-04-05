mod discovery;
mod input;
mod overlay;
#[cfg(test)]
mod tests;

use std::path::Path;

use crate::{
    core::{EntryKind, FileClass},
    file_info::{PreviewKind, inspect_path},
};

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn path_is_text_like(path: &Path) -> bool {
    let facts = inspect_path(path, EntryKind::File);

    match facts.preview.kind {
        PreviewKind::Markdown | PreviewKind::Csv => true,
        // Source previews are usually a good editor fit, but image formats like
        // SVG should still behave like images in "Open With".
        PreviewKind::Source => facts.builtin_class != FileClass::Image,
        // Plain-text previews cover both true text files and some binary
        // document/image categories that render metadata as text. Only treat
        // them as editor-friendly when they are not one of those richer types.
        PreviewKind::PlainText => {
            facts.preview.document_format.is_none()
                && !matches!(
                    facts.builtin_class,
                    FileClass::Image
                        | FileClass::Audio
                        | FileClass::Video
                        | FileClass::Archive
                        | FileClass::Font
                )
        }
        _ => false,
    }
}
