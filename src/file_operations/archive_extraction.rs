use super::FileOperationsState;
use crate::archive::{ArchiveEncryption, ArchivePassword};
use std::{fmt, path::PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct ArchiveExtractProgress {
    pub(crate) completed: usize,
    pub(crate) total: Option<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct ArchiveExtractBatchState {
    pub(crate) total_archives: usize,
    pub(crate) completed_archives: usize,
    pub(crate) failed_archives: usize,
    pub(crate) skipped_archives: usize,
    pub(crate) skipped_non_archives: usize,
    pub(crate) dest_dirs: Vec<PathBuf>,
}

impl ArchiveExtractBatchState {
    pub(crate) fn new(total_archives: usize, skipped_non_archives: usize) -> Self {
        Self {
            total_archives,
            completed_archives: 0,
            failed_archives: 0,
            skipped_archives: 0,
            skipped_non_archives,
            dest_dirs: Vec::new(),
        }
    }

    pub(crate) fn finished_archives(&self) -> usize {
        self.completed_archives + self.failed_archives + self.skipped_archives
    }

    pub(crate) fn is_single_archive(&self) -> bool {
        self.total_archives == 1 && self.skipped_non_archives == 0
    }

    pub(crate) fn reselect_path(&self) -> Option<PathBuf> {
        self.is_single_archive()
            .then(|| self.dest_dirs.first().cloned())
            .flatten()
    }

    pub(crate) fn status(&self) -> String {
        if self.is_single_archive()
            && self.completed_archives == 1
            && let Some(dest_dir) = self.dest_dirs.first()
        {
            let name = dest_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("folder");
            return format!("Extracted 1 archive to \"{name}\"");
        }

        let mut parts = Vec::new();
        parts.push(format!(
            "Extracted {} {}",
            self.completed_archives,
            if self.completed_archives == 1 {
                "archive"
            } else {
                "archives"
            }
        ));
        if self.failed_archives > 0 {
            parts.push(format!("{} failed", self.failed_archives));
        }
        if self.skipped_archives > 0 {
            parts.push(format!("{} skipped", self.skipped_archives));
        }
        if self.skipped_non_archives > 0 {
            parts.push(format!(
                "skipped {} {}",
                self.skipped_non_archives,
                if self.skipped_non_archives == 1 {
                    "non-archive"
                } else {
                    "non-archives"
                }
            ));
        }
        parts.join(", ")
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ArchiveExtractRequest {
    pub(crate) token: u64,
    pub(crate) archives: Vec<PathBuf>,
    pub(crate) password: Option<ArchivePassword>,
    pub(crate) batch: ArchiveExtractBatchState,
}

#[derive(Clone, Debug)]
pub(crate) enum ArchivePasswordPurpose {
    Extract { request: ArchiveExtractRequest },
    Create,
}

#[derive(Clone)]
pub(crate) struct ArchivePasswordOverlay {
    pub(crate) purpose: ArchivePasswordPurpose,
    pub(crate) input: String,
    pub(crate) cursor_col: usize,
    pub(crate) visible: bool,
    pub(crate) error: Option<String>,
}

impl fmt::Debug for ArchivePasswordOverlay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArchivePasswordOverlay")
            .field("purpose", &self.purpose)
            .field("input", &"<redacted>")
            .field("cursor_col", &self.cursor_col)
            .field("visible", &self.visible)
            .field("error", &self.error)
            .finish()
    }
}

pub(crate) enum ArchiveExtractPreparation {
    Ready(ArchiveExtractRequest),
    Rejected(String),
}

pub(crate) enum ArchivePasswordConfirmation {
    None,
    Create { applied: bool },
    Extract(ArchiveExtractRequest),
}

pub(crate) enum ArchivePasswordCancellation {
    None,
    Continue(ArchiveExtractRequest),
    Finished(ArchiveExtractBatchState),
}

pub(crate) struct ArchiveExtractCompletion {
    pub(crate) source_cwd: PathBuf,
    pub(crate) reselect_path: Option<PathBuf>,
    pub(crate) status: String,
}

impl FileOperationsState {
    pub(crate) fn prepare_archive_extract(
        &self,
        paths: Vec<PathBuf>,
        selection_active: bool,
        focused_is_dir: bool,
    ) -> ArchiveExtractPreparation {
        if self.archive_extract_progress.is_some() {
            return ArchiveExtractPreparation::Rejected(
                "Extraction already in progress".to_string(),
            );
        }

        if selection_active {
            let mut archives = Vec::new();
            let mut skipped_non_archives = 0usize;
            for path in paths {
                if path.is_file() && crate::archive::plan_extract(&path).is_ok() {
                    archives.push(path);
                } else {
                    skipped_non_archives += 1;
                }
            }
            if archives.is_empty() {
                return ArchiveExtractPreparation::Rejected("No archives selected".to_string());
            }
            return ArchiveExtractPreparation::Ready(ArchiveExtractRequest {
                token: 0,
                batch: ArchiveExtractBatchState::new(archives.len(), skipped_non_archives),
                archives,
                password: None,
            });
        }

        let Some(archive_path) = paths.into_iter().next() else {
            return ArchiveExtractPreparation::Rejected("Select an archive to extract".to_string());
        };
        if focused_is_dir {
            return ArchiveExtractPreparation::Rejected("Select an archive to extract".to_string());
        }
        if let Err(error) = crate::archive::plan_extract(&archive_path) {
            return ArchiveExtractPreparation::Rejected(error.to_string());
        }
        ArchiveExtractPreparation::Ready(ArchiveExtractRequest {
            token: 0,
            archives: vec![archive_path],
            password: None,
            batch: ArchiveExtractBatchState::new(1, 0),
        })
    }

    pub(crate) fn start_archive_extract(
        &mut self,
        mut request: ArchiveExtractRequest,
        source_cwd: PathBuf,
    ) -> Option<ArchiveExtractRequest> {
        if self.archive_extract_progress.is_some() {
            return None;
        }
        let token = self.archive_extract_token.wrapping_add(1);
        self.archive_extract_token = token;
        request.token = token;
        self.archive_extract_progress = Some(ArchiveExtractProgress {
            completed: request.batch.finished_archives(),
            total: Some(request.batch.total_archives),
        });
        if self.archive_extract_source_cwd.is_none() {
            self.archive_extract_source_cwd = Some(source_cwd);
        }
        self.archive_extract_request = Some(request.clone());
        Some(request)
    }

    pub(crate) fn reject_archive_extract_submission(&mut self) {
        self.archive_extract_progress = None;
        self.archive_extract_source_cwd = None;
        self.archive_extract_request = None;
    }

    pub(crate) fn archive_extract_job_is_current(&self, token: u64) -> bool {
        token == self.archive_extract_token
    }

    pub(crate) fn update_archive_extract_progress(
        &mut self,
        completed: usize,
        total: Option<usize>,
    ) {
        if let Some(progress) = &mut self.archive_extract_progress {
            progress.completed = completed;
            progress.total = total;
        }
    }

    pub(crate) fn finish_archive_extract_job(&mut self) {
        self.archive_extract_progress = None;
    }

    pub(crate) fn remember_archive_extract_request(&mut self, request: ArchiveExtractRequest) {
        self.archive_extract_request = Some(request);
    }

    pub(crate) fn cancel_archive_extract_job(&mut self) -> Option<u64> {
        self.archive_extract_progress.take()?;
        Some(self.archive_extract_token)
    }

    pub(crate) fn archive_password_overlay_mut(&mut self) -> Option<&mut ArchivePasswordOverlay> {
        self.archive_password.as_mut()
    }

    pub(crate) fn open_archive_password_prompt(
        &mut self,
        request: ArchiveExtractRequest,
        error: Option<String>,
    ) {
        self.trash = None;
        self.restore = None;
        self.create = None;
        self.rename = None;
        self.bulk_rename = None;
        self.copy = None;
        self.archive_password = Some(ArchivePasswordOverlay {
            purpose: ArchivePasswordPurpose::Extract { request },
            input: String::new(),
            cursor_col: 0,
            visible: false,
            error,
        });
    }

    pub(crate) fn confirm_archive_password(&mut self) -> ArchivePasswordConfirmation {
        let Some(overlay) = &self.archive_password else {
            return ArchivePasswordConfirmation::None;
        };
        let password = overlay.input.clone();
        if password.is_empty() {
            if let Some(overlay) = &mut self.archive_password {
                overlay.error = Some("Password cannot be empty".to_string());
            }
            return ArchivePasswordConfirmation::None;
        }

        match &overlay.purpose {
            ArchivePasswordPurpose::Extract { request } => {
                let mut request = request.clone();
                request.password = Some(ArchivePassword::new(password));
                ArchivePasswordConfirmation::Extract(request)
            }
            ArchivePasswordPurpose::Create => {
                let applied = if let Some(overlay) = &mut self.archive_create {
                    overlay.options.encryption =
                        ArchiveEncryption::Password(ArchivePassword::new(password));
                    overlay.error = None;
                    true
                } else {
                    false
                };
                self.archive_password = None;
                ArchivePasswordConfirmation::Create { applied }
            }
        }
    }

    pub(crate) fn accept_archive_password(&mut self) {
        self.archive_password = None;
    }

    pub(crate) fn cancel_archive_password_prompt(&mut self) -> ArchivePasswordCancellation {
        let Some(overlay) = self.archive_password.take() else {
            return ArchivePasswordCancellation::None;
        };
        let ArchivePasswordPurpose::Extract { mut request } = overlay.purpose else {
            return ArchivePasswordCancellation::None;
        };
        if !request.archives.is_empty() {
            request.archives.remove(0);
            request.batch.skipped_archives += 1;
        }
        request.password = None;
        if request.archives.is_empty() {
            ArchivePasswordCancellation::Finished(request.batch)
        } else {
            ArchivePasswordCancellation::Continue(request)
        }
    }

    pub(crate) fn finish_archive_extract(
        &mut self,
        source_cwd_fallback: PathBuf,
        status: String,
        reselect_path: Option<PathBuf>,
    ) -> ArchiveExtractCompletion {
        self.archive_extract_progress = None;
        self.archive_extract_request = None;
        ArchiveExtractCompletion {
            source_cwd: self
                .archive_extract_source_cwd
                .take()
                .unwrap_or(source_cwd_fallback),
            reselect_path,
            status,
        }
    }

    pub fn archive_extract_progress(&self) -> Option<(usize, Option<usize>)> {
        self.archive_extract_progress
            .as_ref()
            .map(|progress| (progress.completed, progress.total))
    }

    pub fn archive_password_is_open(&self) -> bool {
        self.archive_password.is_some()
    }

    pub fn archive_password_archive_name(&self) -> String {
        let Some(overlay) = &self.archive_password else {
            return "archive".to_string();
        };
        match &overlay.purpose {
            ArchivePasswordPurpose::Extract { request } => request
                .archives
                .first()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or("archive")
                .to_string(),
            ArchivePasswordPurpose::Create => self.archive_create_input().to_string(),
        }
    }

    pub fn archive_password_title_prefix(&self) -> &'static str {
        let Some(overlay) = &self.archive_password else {
            return "Password for";
        };
        match overlay.purpose {
            ArchivePasswordPurpose::Extract { .. } => "Password for",
            ArchivePasswordPurpose::Create
                if self
                    .archive_create
                    .as_ref()
                    .is_some_and(|create| create.options.encryption.is_password_set()) =>
            {
                "Change password for"
            }
            ArchivePasswordPurpose::Create => "New password for",
        }
    }

    pub fn archive_password_placeholder(&self) -> &'static str {
        let Some(overlay) = &self.archive_password else {
            return "password…";
        };
        match overlay.purpose {
            ArchivePasswordPurpose::Extract { .. } => "password…",
            ArchivePasswordPurpose::Create => "new password…",
        }
    }

    pub fn archive_password_input(&self) -> &str {
        self.archive_password
            .as_ref()
            .map_or("", |overlay| &overlay.input)
    }

    pub fn archive_password_cursor_col(&self) -> usize {
        self.archive_password
            .as_ref()
            .map_or(0, |overlay| overlay.cursor_col)
    }

    pub fn archive_password_error(&self) -> Option<&str> {
        self.archive_password
            .as_ref()
            .and_then(|overlay| overlay.error.as_deref())
    }

    pub fn archive_password_is_visible(&self) -> bool {
        self.archive_password
            .as_ref()
            .is_some_and(|overlay| overlay.visible)
    }

    pub(crate) fn toggle_archive_password_visibility(&mut self) {
        if let Some(overlay) = &mut self.archive_password {
            overlay.visible = !overlay.visible;
        }
    }
}
