mod archive_creation;
mod archive_extraction;
mod bulk_rename;
mod copy_move;
mod copy_to_clipboard;
mod create;
mod editor_bulk_rename;
mod file_operations_state;
mod rename;
mod restore;
mod symlink_creation;
mod trash_delete;
mod trash_restoration;

pub(crate) use archive_creation::{ArchiveCreateOverlay, ArchiveCreateProgress};
pub(crate) use archive_extraction::{ArchiveExtractProgress, ArchivePasswordOverlay};
pub(crate) use bulk_rename::{BulkRenameItem, BulkRenameOverlay};
pub(crate) use copy_move::{ClipOp, Clipboard, PasteOrigin, PasteProgress, QueuedPaste};
pub(crate) use copy_to_clipboard::CopyOverlay;
pub(crate) use create::CreateOverlay;
#[cfg(unix)]
pub(crate) use editor_bulk_rename::BulkRenameEditorSession;
pub(crate) use editor_bulk_rename::EditorRenameConfirmOverlay;
pub(crate) use file_operations_state::FileOperationsState;
pub(crate) use rename::RenameOverlay;
pub(crate) use restore::{RestoreOverlay, RestoreProgress};
pub(crate) use trash_delete::{TrashOverlay, TrashProgress, TrashTarget};
pub(crate) use trash_restoration::restore_trash_item;
#[cfg(target_os = "macos")]
pub(crate) use trash_restoration::{
    remove_restore_origins, remove_restore_origins_checked, restore_trash_item_checked_metadata,
    save_restore_origins_checked,
};

#[cfg(test)]
mod tests;
