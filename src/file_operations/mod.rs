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

pub(crate) use archive_creation::{
    ArchiveCreateOverlay, ArchiveCreateProgress, ArchiveCreateRequest,
};
#[cfg(test)]
pub(crate) use archive_extraction::ArchiveExtractBatchState;
pub(crate) use archive_extraction::{
    ArchiveExtractPreparation, ArchiveExtractProgress, ArchiveExtractRequest,
    ArchivePasswordCancellation, ArchivePasswordConfirmation, ArchivePasswordOverlay,
};
pub(crate) use bulk_rename::{BulkRenameItem, BulkRenameOverlay};
pub(crate) use copy_move::{
    ClipOp, Clipboard, PasteOrigin, PasteProgress, PasteRequest, QueuedPaste,
};
pub(crate) use copy_to_clipboard::CopyOverlay;
pub(crate) use create::CreateOverlay;
pub(crate) use editor_bulk_rename::{
    BulkRenameConfirmation, EditorRenameConfirmOverlay, EditorRenameReview,
};
#[cfg(unix)]
pub(crate) use editor_bulk_rename::{BulkRenameEditorSession, EditorBulkRenameLaunch};
pub(crate) use file_operations_state::FileOperationsState;
pub(crate) use rename::RenameOverlay;
pub(crate) use restore::{RestoreConfirmation, RestoreOverlay, RestoreProgress, RestoreRequest};
pub(crate) use trash_delete::{
    TrashConfirmation, TrashTargetScope, likely_cross_device_trash, trash_target_from_path,
    trash_target_is_inside_trash, trash_target_scope,
};
pub(crate) use trash_delete::{TrashOverlay, TrashProgress, TrashRequest, TrashTarget};

#[cfg(test)]
mod tests;
