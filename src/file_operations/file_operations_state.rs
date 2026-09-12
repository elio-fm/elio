use std::{collections::VecDeque, path::PathBuf};

use crate::background_jobs::job_requests::ArchiveExtractRequest;

use super::{
    ArchiveCreateOverlay, ArchiveCreateProgress, ArchiveExtractProgress, ArchivePasswordOverlay,
    BulkRenameOverlay, Clipboard, CopyOverlay, CreateOverlay, EditorRenameConfirmOverlay,
    PasteProgress, QueuedPaste, RenameOverlay, RestoreOverlay, RestoreProgress, TrashOverlay,
    TrashProgress,
};

#[derive(Default)]
pub(crate) struct FileOperationsState {
    pub(crate) trash: Option<TrashOverlay>,
    pub(crate) restore: Option<RestoreOverlay>,
    pub(crate) archive_create: Option<ArchiveCreateOverlay>,
    pub(crate) archive_password: Option<ArchivePasswordOverlay>,
    pub(crate) create: Option<CreateOverlay>,
    pub(crate) rename: Option<RenameOverlay>,
    pub(crate) bulk_rename: Option<BulkRenameOverlay>,
    pub(crate) editor_rename_confirm: Option<EditorRenameConfirmOverlay>,
    pub(crate) copy: Option<CopyOverlay>,
    pub(crate) clipboard: Option<Clipboard>,
    pub(crate) archive_create_token: u64,
    pub(crate) archive_create_progress: Option<ArchiveCreateProgress>,
    pub(crate) archive_create_source_cwd: Option<PathBuf>,
    pub(crate) archive_create_path: Option<PathBuf>,
    pub(crate) archive_extract_token: u64,
    pub(crate) archive_extract_progress: Option<ArchiveExtractProgress>,
    pub(crate) archive_extract_source_cwd: Option<PathBuf>,
    pub(crate) archive_extract_request: Option<ArchiveExtractRequest>,
    pub(crate) paste_token: u64,
    pub(crate) paste_progress: Option<PasteProgress>,
    pub(crate) queued_pastes: VecDeque<QueuedPaste>,
    /// Destination directory of the in-flight paste. Kept separately from
    /// `paste_progress` so that cancelling the chip does not lose the context
    /// needed by the completion handler to reload the right directory.
    pub(crate) paste_dest_dir: Option<PathBuf>,
    pub(crate) trash_token: u64,
    pub(crate) trash_progress: Option<TrashProgress>,
    /// Source directory of the in-flight trash. Kept separately from
    /// `trash_progress` for the same reason as `paste_dest_dir`.
    pub(crate) trash_source_cwd: Option<PathBuf>,
    pub(crate) restore_token: u64,
    pub(crate) restore_progress: Option<RestoreProgress>,
    /// Source directory of the in-flight restore. Kept separately from
    /// `restore_progress` so that cancelling the chip does not lose the
    /// context needed by the completion handler.
    pub(crate) restore_source_cwd: Option<PathBuf>,
}
