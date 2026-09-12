use std::{collections::VecDeque, path::PathBuf};

use super::{
    ArchiveCreateOverlay, ArchiveCreateProgress, ArchiveExtractProgress, ArchiveExtractRequest,
    ArchivePasswordOverlay, BulkRenameOverlay, Clipboard, CopyOverlay, CreateOverlay,
    EditorRenameConfirmOverlay, PasteProgress, QueuedPaste, RenameOverlay, RestoreOverlay,
    RestoreProgress, TrashOverlay, TrashProgress,
};

#[derive(Default)]
pub(crate) struct FileOperationsState {
    pub(super) trash: Option<TrashOverlay>,
    pub(super) restore: Option<RestoreOverlay>,
    pub(super) archive_create: Option<ArchiveCreateOverlay>,
    pub(super) archive_password: Option<ArchivePasswordOverlay>,
    pub(super) create: Option<CreateOverlay>,
    pub(super) rename: Option<RenameOverlay>,
    pub(super) bulk_rename: Option<BulkRenameOverlay>,
    pub(super) editor_rename_confirm: Option<EditorRenameConfirmOverlay>,
    pub(super) copy: Option<CopyOverlay>,
    pub(super) clipboard: Option<Clipboard>,
    pub(super) archive_create_token: u64,
    pub(super) archive_create_progress: Option<ArchiveCreateProgress>,
    pub(super) archive_create_source_cwd: Option<PathBuf>,
    pub(super) archive_create_path: Option<PathBuf>,
    pub(super) archive_extract_token: u64,
    pub(super) archive_extract_progress: Option<ArchiveExtractProgress>,
    pub(super) archive_extract_source_cwd: Option<PathBuf>,
    pub(super) archive_extract_request: Option<ArchiveExtractRequest>,
    pub(super) paste_token: u64,
    pub(super) paste_progress: Option<PasteProgress>,
    pub(super) queued_pastes: VecDeque<QueuedPaste>,
    /// Destination directory of the in-flight paste. Kept separately from
    /// `paste_progress` so that cancelling the chip does not lose the context
    /// needed by the completion handler to reload the right directory.
    pub(super) paste_dest_dir: Option<PathBuf>,
    pub(super) trash_token: u64,
    pub(super) trash_progress: Option<TrashProgress>,
    /// Source directory of the in-flight trash. Kept separately from
    /// `trash_progress` for the same reason as `paste_dest_dir`.
    pub(super) trash_source_cwd: Option<PathBuf>,
    pub(super) restore_token: u64,
    pub(super) restore_progress: Option<RestoreProgress>,
    /// Source directory of the in-flight restore. Kept separately from
    /// `restore_progress` so that cancelling the chip does not lose the
    /// context needed by the completion handler.
    pub(super) restore_source_cwd: Option<PathBuf>,
}

impl FileOperationsState {
    pub(crate) fn has_active_job(&self) -> bool {
        self.trash_progress.is_some()
            || self.restore_progress.is_some()
            || self.archive_create_progress.is_some()
            || self.archive_extract_progress.is_some()
            || self.paste_progress.is_some()
    }

    pub(crate) fn overlay_blocks_terminal_images(&self) -> bool {
        self.trash.is_some()
            || self.restore.is_some()
            || self.archive_password.is_some()
            || self.archive_create.is_some()
            || self.create.is_some()
            || self.rename.is_some()
            || self.bulk_rename.is_some()
            || self.copy.is_some()
    }
}
