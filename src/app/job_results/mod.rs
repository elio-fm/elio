use super::*;
use crate::background_jobs::job_results::*;
use crate::preview;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

const JOB_RESULT_APPLY_MAX_PER_TICK: usize = 12;
const JOB_RESULT_APPLY_TIME_BUDGET: Duration = Duration::from_millis(2);

impl App {
    fn refresh_static_image_preloads_for_cached_preview_visual(
        &mut self,
        build_entry: &Entry,
        build_variant: &preview::PreviewRequestOptions,
        build_visual_kind: Option<preview::PreviewVisualKind>,
        is_current_entry: bool,
    ) {
        let Some(build_visual_kind) = build_visual_kind else {
            return;
        };

        let should_refresh = match build_visual_kind {
            preview::PreviewVisualKind::PageImage => {
                is_current_entry
                    || self.refreshes_image_preloads_for_nearby_comic_entry_preview(
                        build_entry,
                        build_variant,
                    )
                    || self.refreshes_image_preloads_for_nearby_epub_entry_preview(
                        build_entry,
                        build_variant,
                    )
            }
            preview::PreviewVisualKind::Cover => {
                is_current_entry
                    || self.refreshes_image_preloads_for_nearby_audio_preview(
                        build_entry,
                        build_variant,
                    )
            }
        };
        if should_refresh {
            self.refresh_static_image_preloads();
        }
    }

    pub fn process_background_jobs(&mut self) -> bool {
        let mut dirty = false;
        let started_at = Instant::now();
        let mut processed = 0usize;

        while processed < JOB_RESULT_APPLY_MAX_PER_TICK
            && started_at.elapsed() < JOB_RESULT_APPLY_TIME_BUDGET
        {
            let Ok(job) = self.job_scheduler.try_recv() else {
                break;
            };
            processed += 1;
            match job {
                JobResult::Directory(build) => {
                    let Some(load) = self.file_browser.directory_runtime.pending_load.clone()
                    else {
                        continue;
                    };
                    if build.token != self.file_browser.directory_runtime.load_token
                        || build.token != load.token
                        || build.cwd != load.target_cwd
                    {
                        continue;
                    }

                    self.file_browser.directory_runtime.pending_load = None;
                    dirty = true;

                    match build.result {
                        Ok(snapshot) => self.apply_directory_snapshot(load, snapshot),
                        Err(error) => {
                            self.preview.exit_fullscreen_after_directory_load = false;
                            self.exit_fullscreen_preview();
                            self.status = format!("Cannot open {}: {}", build.cwd.display(), error);
                        }
                    }
                }
                JobResult::DirectoryFingerprint(build) => {
                    let Some(scan) = self
                        .file_browser
                        .directory_runtime
                        .pending_fingerprint_scan
                        .clone()
                    else {
                        continue;
                    };
                    if build.token != self.file_browser.directory_runtime.fingerprint_token
                        || build.token != scan.token
                        || build.cwd != scan.cwd
                        || build.show_hidden != scan.show_hidden
                    {
                        continue;
                    }

                    self.file_browser.directory_runtime.pending_fingerprint_scan = None;

                    let Ok(fingerprint) = build.result else {
                        continue;
                    };
                    if self.file_browser.directory_runtime.pending_load.is_some()
                        || fingerprint == self.file_browser.directory_runtime.fingerprint
                    {
                        continue;
                    }
                    if self.queue_directory_reload(true).is_ok() {
                        dirty = true;
                    }
                }
                JobResult::DirectoryItemCount(build) => {
                    self.cache_directory_item_count(
                        build.path.clone(),
                        build.modified,
                        build.show_hidden,
                        build.item_count,
                    );
                    dirty |= self.should_redraw_for_directory_item_count(
                        &build.path,
                        build.modified,
                        build.show_hidden,
                    );
                }
                JobResult::DirectoryStats(build) => {
                    dirty |= self.apply_preview_directory_stats_result(
                        build.token,
                        &build.path,
                        build.result,
                    );
                }
                JobResult::GitStatus(build) => {
                    dirty |= self.file_browser.apply_git_status(
                        build.token,
                        build.cwd,
                        build.branch,
                        build.dirty,
                    );
                }
                JobResult::PreviewLineCount(build) => {
                    dirty |= self.apply_preview_line_count_result(
                        &build.path,
                        build.size,
                        build.modified,
                        build.total_lines,
                    );
                }
                JobResult::PdfProbe(build) => {
                    dirty |= self.apply_pdf_probe_build(build);
                }
                JobResult::PdfRender(build) => {
                    dirty |= self.apply_pdf_render_build(build);
                }
                JobResult::ImagePrepare(build) => {
                    dirty |= self.apply_image_prepare_build(build);
                }
                JobResult::SearchBatch(build) => {
                    if build.token != self.fuzzy_finder.token
                        || build.cwd != self.file_browser.cwd
                        || build.show_hidden != self.effective_show_hidden()
                        || build.fingerprint != self.file_browser.directory_runtime.fingerprint
                    {
                        continue;
                    }

                    let mut sync_search_scroll = false;
                    if let Some(search) = &mut self.fuzzy_finder.search
                        && search.scope == build.scope
                    {
                        search.loading = true;
                        search.error = None;
                        search.stats = build.batch.stats;
                        if !build.batch.candidates.is_empty() {
                            search.append_candidates(build.batch.candidates);
                            sync_search_scroll = true;
                        }
                        dirty = true;
                    }
                    if sync_search_scroll {
                        self.sync_search_scroll();
                    }
                }
                JobResult::Search(build) => {
                    if build.token != self.fuzzy_finder.token
                        || build.cwd != self.file_browser.cwd
                        || build.show_hidden != self.effective_show_hidden()
                        || build.fingerprint != self.file_browser.directory_runtime.fingerprint
                    {
                        continue;
                    }

                    self.fuzzy_finder.loading = false;
                    dirty = true;

                    match build.result {
                        Ok(index) => {
                            let stats = index.stats;
                            let candidates = Arc::new(index.candidates);
                            self.fuzzy_finder.cache = Some(SearchCache {
                                cwd: build.cwd,
                                scope: build.scope,
                                show_hidden: build.show_hidden,
                                fingerprint: build.fingerprint,
                                candidates: candidates.clone(),
                                stats,
                            });
                            if let Some(search) = &mut self.fuzzy_finder.search
                                && search.scope == build.scope
                            {
                                search.replace_candidates(candidates, stats);
                            }
                            self.sync_search_scroll();
                        }
                        Err(error) => {
                            self.fuzzy_finder.cache = None;
                            if let Some(search) = &mut self.fuzzy_finder.search
                                && search.scope == build.scope
                            {
                                search.fail_loading(error);
                            }
                        }
                    }
                }
                JobResult::DuplicateScanBatch(build) => {
                    if build.token != self.duplicate_finder.scan_token
                        || build.cwd != self.file_browser.cwd
                        || build.show_hidden != self.effective_show_hidden()
                    {
                        continue;
                    }
                    self.apply_duplicate_batch(build.batch);
                    dirty = true;
                }
                JobResult::DuplicateScan(build) => {
                    if build.token != self.duplicate_finder.scan_token
                        || build.cwd != self.file_browser.cwd
                        || build.show_hidden != self.effective_show_hidden()
                    {
                        continue;
                    }
                    self.apply_duplicate_result(build.result);
                    dirty = true;
                }
                JobResult::ArchiveCreate(build) => {
                    if !self
                        .file_operations
                        .archive_create_job_is_current(build.token)
                    {
                        continue;
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
                        if nav_to_source
                            || (source_cwd == self.file_browser.cwd && nav_target.is_none())
                        {
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
                    dirty = true;
                }
                JobResult::ArchiveExtract(build) => {
                    if !self
                        .file_operations
                        .archive_extract_job_is_current(build.token)
                    {
                        continue;
                    }
                    if build.done {
                        self.file_operations.finish_archive_extract_job();
                        if let Some(prompt) = build.password_prompt {
                            if let Some(request) = build.password_request {
                                let error = match prompt {
                                    ArchivePasswordPrompt::Required => None,
                                    ArchivePasswordPrompt::BadPassword => {
                                        Some("Wrong password".to_string())
                                    }
                                };
                                self.file_operations
                                    .remember_archive_extract_request(request.clone());
                                self.open_archive_password_prompt(request, error);
                            } else {
                                self.status = "Archive requires a password".to_string();
                            }
                            dirty = true;
                            continue;
                        }
                        self.finish_archive_extract(
                            build.status.unwrap_or_default(),
                            build.dest_dir,
                        );
                    } else {
                        self.file_operations
                            .update_archive_extract_progress(build.completed, build.total);
                    }
                    dirty = true;
                }
                JobResult::Paste(build) => {
                    if !self.file_operations.paste_job_is_current(build.token) {
                        continue;
                    }
                    if build.done {
                        let completion = self.file_operations.finish_paste_job();
                        let dest_dir = completion
                            .dest_dir
                            .unwrap_or_else(|| self.file_browser.cwd.clone());
                        let status = build.status.unwrap_or_default();
                        let defer_reload_for_same_dest =
                            completion.next_queued_dest.as_deref() == Some(dest_dir.as_path());
                        // Only reload in-place when the user is still in the
                        // destination directory and not mid-navigation to
                        // somewhere else (which would cancel their navigation).
                        let nav_target = self
                            .file_browser
                            .directory_runtime
                            .pending_load
                            .as_ref()
                            .map(|l| l.target_cwd.as_path());
                        let nav_to_dest = nav_target == Some(dest_dir.as_path());
                        if dest_dir == self.file_browser.cwd
                            && (nav_target.is_none() || nav_to_dest)
                            && !defer_reload_for_same_dest
                        {
                            let reselect_path = if completion.origin
                                == Some(crate::file_operations::PasteOrigin::Drop)
                            {
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
                            // User navigated away — just surface the status.
                            // Navigation will load dest_dir fresh if they return.
                            // If another queued paste targets this same
                            // directory, defer the reload until that queued
                            // paste finishes to avoid showing a mid-queue
                            // snapshot.
                            self.status = status;
                        }
                        if let Some(request) = self.file_operations.start_next_queued_paste() {
                            self.job_scheduler.submit_paste(request);
                        }
                    } else {
                        self.file_operations.update_paste_progress(build.completed);
                    }
                    dirty = true;
                }
                JobResult::Trash(build) => {
                    if !self.file_operations.trash_job_is_current(build.token) {
                        continue;
                    }
                    if build.done {
                        // Only reposition the cursor when every target was
                        // actually removed.  Cancelled or partially-failed
                        // operations leave some entries intact, so using the
                        // pre-computed survivor path would move the cursor
                        // away from entries that are still present.
                        let completion = self.file_operations.finish_trash_job(build.completed);
                        let source_cwd = completion
                            .source_cwd
                            .unwrap_or_else(|| self.file_browser.cwd.clone());
                        let status = build.status.unwrap_or_default();
                        if let Some(paths) = completion.duplicate_targets.as_ref() {
                            self.remove_duplicate_paths(paths);
                        }
                        // Only reload in-place when the user is still in the
                        // source directory and not mid-navigation to somewhere
                        // else (which would cancel their navigation).
                        let nav_target = self
                            .file_browser
                            .directory_runtime
                            .pending_load
                            .as_ref()
                            .map(|l| l.target_cwd.as_path());
                        let nav_to_source = nav_target == Some(source_cwd.as_path());
                        if nav_to_source
                            || (source_cwd == self.file_browser.cwd && nav_target.is_none())
                        {
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
                            // User navigated away — just surface the status.
                            // Navigation will load source_cwd fresh if they return.
                            self.status = status;
                        }
                    } else {
                        self.file_operations.update_trash_progress(build.completed);
                    }
                    dirty = true;
                }
                JobResult::Restore(build) => {
                    if !self.file_operations.restore_job_is_current(build.token) {
                        continue;
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
                            .map(|l| l.target_cwd.as_path());
                        let nav_to_source = nav_target == Some(source_cwd.as_path());
                        if nav_to_source
                            || (source_cwd == self.file_browser.cwd && nav_target.is_none())
                        {
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
                    dirty = true;
                }
                JobResult::Preview(build) => {
                    self.preview.state.remember_preview(
                        &build.entry,
                        &build.variant,
                        build.code_line_limit,
                        build.code_render_limit,
                        build.ffmpeg_available,
                        &build.result,
                    );
                    let build_is_comic = build.result.kind == preview::PreviewKind::Comic;
                    let build_is_epub_section = matches!(
                        build.variant,
                        preview::PreviewRequestOptions::EpubSection(_)
                    );
                    let build_visual_kind = build
                        .result
                        .preview_visual
                        .as_ref()
                        .map(|visual| visual.kind);
                    let is_current_entry = self
                        .active_preview_entry()
                        .map(|entry| {
                            entry.path == build.entry.path
                                && entry.modified == build.entry.modified
                                && entry.size == build.entry.size
                        })
                        .unwrap_or(false);
                    let is_current_variant =
                        build.variant == self.current_preview_request_options();
                    if build.token != self.preview.state.token
                        || !is_current_entry
                        || !is_current_variant
                        || build.code_line_limit
                            != self.preview_code_line_limit_for_entry(&build.entry)
                    {
                        // For comic results that match the current entry and variant but
                        // arrived with a stale token, apply them immediately if we are
                        // still showing a placeholder (no preview at all).  The rendered
                        // page list is deterministic for a given path + page index, so a
                        // token skew does not indicate wrong content.  This rescues the
                        // common race where a rapid-nav `refresh_preview()` bumps the
                        // token after the job was already submitted, and the result
                        // arrives before the replacement job finishes — leaving the
                        // placeholder on-screen even though a valid result is available.
                        let can_rescue_stale_comic = build_is_comic
                            && is_current_entry
                            && is_current_variant
                            && build.code_line_limit
                                == self.preview_code_line_limit_for_entry(&build.entry)
                            && matches!(
                                &self.preview.state.load_state,
                                Some(PreviewLoadState::Placeholder(p) | PreviewLoadState::Refreshing(p))
                                    if p == &build.entry.path
                            );
                        if !can_rescue_stale_comic {
                            self.refresh_static_image_preloads_for_cached_preview_visual(
                                &build.entry,
                                &build.variant,
                                build_visual_kind,
                                is_current_entry,
                            );
                            // Clear the in-flight flag if this stale drop belongs to our
                            // outstanding extension job (prevents stuck state when the
                            // user navigates away mid-extension).
                            if self.preview.state.incremental_render_path.as_deref()
                                == Some(build.entry.path.as_path())
                            {
                                self.preview.state.incremental_render_in_flight = false;
                                self.preview.state.incremental_render_path = None;
                            }
                            self.preview.state.metrics.stale_results_dropped += 1;
                            continue;
                        }
                    }

                    // Detect whether this is a complete extension result arriving for a
                    // currently-displayed partial preview.  If so, replace the content
                    // WITHOUT resetting scroll so there are no visual artifacts.
                    let is_extension_result = build.result.incremental_render_limit.is_none()
                        && self.preview.state.content.is_incrementally_partial()
                        && is_current_entry
                        && is_current_variant;

                    self.preview.state.content = build.result;
                    if self.preview.state.content.kind != preview::PreviewKind::Directory {
                        self.clear_preview_directory_stats();
                    }
                    self.preview.state.load_state = None;
                    self.apply_current_comic_preview_metadata();
                    self.apply_current_epub_preview_metadata();
                    self.sync_current_preview_line_count();

                    if is_extension_result {
                        // Extension result: preserve scroll, just clamp if needed.
                        self.preview.state.incremental_render_in_flight = false;
                        self.preview.state.incremental_render_path = None;
                        self.sync_preview_scroll();
                    } else {
                        // Normal first-time render: reset scroll.
                        self.preview.state.scroll = 0;
                        self.preview.state.horizontal_scroll = 0;
                        self.sync_preview_scroll();
                    }

                    if build_visual_kind.is_some() {
                        self.refresh_static_image_preloads();
                    }
                    if build_is_comic || build_is_epub_section || is_current_entry {
                        self.prefetch_nearby_audio_previews();
                        self.schedule_preview_prefetch();
                    }
                    self.preview.state.metrics.applied_results += 1;
                    dirty = true;
                }
            }
        }

        if (processed == JOB_RESULT_APPLY_MAX_PER_TICK
            || (processed > 0 && started_at.elapsed() >= JOB_RESULT_APPLY_TIME_BUDGET))
            && let Ok(job) = self.job_scheduler.try_recv()
        {
            self.job_scheduler.defer_result(job);
        }

        dirty
    }
}

#[cfg(test)]
mod tests;
