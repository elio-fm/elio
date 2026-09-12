use super::super::App;
use crate::background_jobs::job_results::{
    ArchiveCreateBuild, ArchiveExtractBuild, ArchivePasswordPrompt, PasteBuild, RestoreBuild,
    TrashBuild,
};
use crate::file_browser::{DirectoryHistoryMode, DirectoryLoadCompletion, PendingDirectoryLoad};

impl App {
    pub(super) fn apply_archive_create_job_result(&mut self, build: ArchiveCreateBuild) -> bool {
        if !self
            .file_operations
            .archive_create_job_is_current(build.token)
        {
            return false;
        }
        if build.done {
            let source_cwd = self
                .file_operations
                .finish_archive_create_job()
                .unwrap_or_else(|| self.file_browser.cwd.clone());
            let status = build.status.unwrap_or_default();
            let nav_target = self
                .file_browser
                .directory_runtime
                .pending_load
                .as_ref()
                .map(|load| load.target_cwd.as_path());
            let nav_to_source = nav_target == Some(source_cwd.as_path());
            if nav_to_source || (source_cwd == self.file_browser.cwd && nav_target.is_none()) {
                let _ = self.queue_directory_load(PendingDirectoryLoad {
                    token: 0,
                    target_cwd: source_cwd,
                    previous_cwd: self.file_browser.cwd.clone(),
                    previous_selected_path: None,
                    previous_selection_name: None,
                    reselect_path: build.output_path,
                    history_mode: DirectoryHistoryMode::None,
                    refresh_search: false,
                    completion: DirectoryLoadCompletion::Status(status),
                });
            } else {
                self.status = status;
            }
        } else {
            self.file_operations
                .update_archive_create_progress(build.completed, build.total);
        }
        true
    }

    pub(super) fn apply_archive_extract_job_result(&mut self, build: ArchiveExtractBuild) -> bool {
        if !self
            .file_operations
            .archive_extract_job_is_current(build.token)
        {
            return false;
        }
        if build.done {
            self.file_operations.finish_archive_extract_job();
            if let Some(prompt) = build.password_prompt {
                if let Some(request) = build.password_request {
                    let error = match prompt {
                        ArchivePasswordPrompt::Required => None,
                        ArchivePasswordPrompt::BadPassword => Some("Wrong password".to_string()),
                    };
                    self.file_operations
                        .remember_archive_extract_request(request.clone());
                    self.open_archive_password_prompt(request, error);
                } else {
                    self.status = "Archive requires a password".to_string();
                }
                return true;
            }
            self.finish_archive_extract(build.status.unwrap_or_default(), build.dest_dir);
        } else {
            self.file_operations
                .update_archive_extract_progress(build.completed, build.total);
        }
        true
    }

    pub(super) fn apply_paste_job_result(&mut self, build: PasteBuild) -> bool {
        if !self.file_operations.paste_job_is_current(build.token) {
            return false;
        }
        if build.done {
            let completion = self.file_operations.finish_paste_job();
            let dest_dir = completion
                .dest_dir
                .unwrap_or_else(|| self.file_browser.cwd.clone());
            let status = build.status.unwrap_or_default();
            let defer_reload_for_same_dest =
                completion.next_queued_dest.as_deref() == Some(dest_dir.as_path());
            // Only reload in-place when the user is still in the destination directory and not
            // mid-navigation to somewhere else, which would cancel their navigation.
            let nav_target = self
                .file_browser
                .directory_runtime
                .pending_load
                .as_ref()
                .map(|load| load.target_cwd.as_path());
            let nav_to_dest = nav_target == Some(dest_dir.as_path());
            if dest_dir == self.file_browser.cwd
                && (nav_target.is_none() || nav_to_dest)
                && !defer_reload_for_same_dest
            {
                let reselect_path =
                    if completion.origin == Some(crate::file_operations::PasteOrigin::Drop) {
                        build.destination_paths.first().cloned()
                    } else {
                        None
                    };
                let _ = self.queue_directory_load(PendingDirectoryLoad {
                    token: 0,
                    target_cwd: dest_dir,
                    previous_cwd: self.file_browser.cwd.clone(),
                    previous_selected_path: None,
                    previous_selection_name: None,
                    reselect_path,
                    history_mode: DirectoryHistoryMode::None,
                    refresh_search: false,
                    completion: DirectoryLoadCompletion::Status(status),
                });
            } else {
                // If the user navigated away, only surface the status. Navigation will load the
                // destination fresh if they return. If another queued paste targets this same
                // directory, defer the reload until that paste finishes.
                self.status = status;
            }
            if let Some(request) = self.file_operations.start_next_queued_paste() {
                self.job_scheduler.submit_paste(request);
            }
        } else {
            self.file_operations.update_paste_progress(build.completed);
        }
        true
    }

    pub(super) fn apply_trash_job_result(&mut self, build: TrashBuild) -> bool {
        if !self.file_operations.trash_job_is_current(build.token) {
            return false;
        }
        if build.done {
            // Only reposition the cursor when every target was actually removed. Cancelled or
            // partially-failed operations leave some entries intact, so using the pre-computed
            // survivor path would move the cursor away from entries that are still present.
            let completion = self.file_operations.finish_trash_job(build.completed);
            let source_cwd = completion
                .source_cwd
                .unwrap_or_else(|| self.file_browser.cwd.clone());
            let status = build.status.unwrap_or_default();
            if let Some(paths) = completion.duplicate_targets.as_ref() {
                self.remove_duplicate_paths(paths);
            }
            // Only reload in-place when the user is still in the source directory and not
            // mid-navigation to somewhere else, which would cancel their navigation.
            let nav_target = self
                .file_browser
                .directory_runtime
                .pending_load
                .as_ref()
                .map(|load| load.target_cwd.as_path());
            let nav_to_source = nav_target == Some(source_cwd.as_path());
            if nav_to_source || (source_cwd == self.file_browser.cwd && nav_target.is_none()) {
                let _ = self.queue_directory_load(PendingDirectoryLoad {
                    token: 0,
                    target_cwd: source_cwd,
                    previous_cwd: self.file_browser.cwd.clone(),
                    previous_selected_path: None,
                    previous_selection_name: None,
                    reselect_path: completion.next_selection,
                    history_mode: DirectoryHistoryMode::None,
                    refresh_search: false,
                    completion: DirectoryLoadCompletion::Status(status),
                });
            } else {
                // If the user navigated away, only surface the status. Navigation will load the
                // source directory fresh if they return.
                self.status = status;
            }
        } else {
            self.file_operations.update_trash_progress(build.completed);
        }
        true
    }

    pub(super) fn apply_restore_job_result(&mut self, build: RestoreBuild) -> bool {
        if !self.file_operations.restore_job_is_current(build.token) {
            return false;
        }
        if build.done {
            let completion = self.file_operations.finish_restore_job(build.completed);
            let source_cwd = completion
                .source_cwd
                .unwrap_or_else(|| self.file_browser.cwd.clone());
            let status = build.status.unwrap_or_default();
            let nav_target = self
                .file_browser
                .directory_runtime
                .pending_load
                .as_ref()
                .map(|load| load.target_cwd.as_path());
            let nav_to_source = nav_target == Some(source_cwd.as_path());
            if nav_to_source || (source_cwd == self.file_browser.cwd && nav_target.is_none()) {
                let _ = self.queue_directory_load(PendingDirectoryLoad {
                    token: 0,
                    target_cwd: source_cwd,
                    previous_cwd: self.file_browser.cwd.clone(),
                    previous_selected_path: None,
                    previous_selection_name: None,
                    reselect_path: completion.next_selection,
                    history_mode: DirectoryHistoryMode::None,
                    refresh_search: false,
                    completion: DirectoryLoadCompletion::Status(status),
                });
            } else {
                self.status = status;
            }
        } else {
            self.file_operations
                .update_restore_progress(build.completed);
        }
        true
    }
}
