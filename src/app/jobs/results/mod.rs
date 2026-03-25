use super::*;
use std::{
    collections::HashMap,
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
            let Ok(job) = self.scheduler.try_recv() else {
                break;
            };
            processed += 1;
            match job {
                JobResult::Directory(build) => {
                    let Some(load) = self.directory_runtime.pending_load.clone() else {
                        continue;
                    };
                    if build.token != self.directory_token
                        || build.token != load.token
                        || build.cwd != load.target_cwd
                    {
                        continue;
                    }

                    self.directory_runtime.pending_load = None;
                    dirty = true;

                    match build.result {
                        Ok(snapshot) => self.apply_directory_snapshot(load, snapshot),
                        Err(error) => {
                            self.status = format!("Cannot open {}: {}", build.cwd.display(), error);
                        }
                    }
                }
                JobResult::DirectoryFingerprint(build) => {
                    let Some(scan) = self.directory_runtime.pending_fingerprint_scan.clone() else {
                        continue;
                    };
                    if build.token != self.directory_fingerprint_token
                        || build.token != scan.token
                        || build.cwd != scan.cwd
                        || build.show_hidden != scan.show_hidden
                    {
                        continue;
                    }

                    self.directory_runtime.pending_fingerprint_scan = None;

                    let Ok(fingerprint) = build.result else {
                        continue;
                    };
                    if self.directory_runtime.pending_load.is_some()
                        || fingerprint == self.directory_runtime.fingerprint
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
                JobResult::Search(build) => {
                    if build.token != self.search_token
                        || build.cwd != self.cwd
                        || build.show_hidden != self.show_hidden
                    {
                        continue;
                    }

                    self.search_loading = false;
                    dirty = true;

                    match build.result {
                        Ok(candidates) => {
                            self.search_cache = Some(SearchCache {
                                cwd: build.cwd,
                                scope: build.scope,
                                show_hidden: build.show_hidden,
                                candidates: candidates.clone(),
                            });
                            if let Some(search) = &mut self.search
                                && search.scope == build.scope
                            {
                                search.candidates = candidates;
                                search.cached_matches = HashMap::from([(
                                    String::new(),
                                    (0..search.candidates.len()).collect(),
                                )]);
                                search.loading = false;
                                search.error = None;
                            }
                            self.refresh_search_matches("");
                        }
                        Err(error) => {
                            self.search_cache = None;
                            if let Some(search) = &mut self.search
                                && search.scope == build.scope
                            {
                                search.candidates = Arc::new(Vec::new());
                                search.matches.clear();
                                search.cached_matches =
                                    HashMap::from([(String::new(), Vec::new())]);
                                search.selected = 0;
                                search.scroll = 0;
                                search.loading = false;
                                search.error = Some(error);
                            }
                        }
                    }
                }
                JobResult::Paste(build) => {
                    if build.token != self.paste_token {
                        continue;
                    }
                    if build.done {
                        self.paste_progress = None;
                        let dest_dir = self
                            .paste_dest_dir
                            .take()
                            .unwrap_or_else(|| self.cwd.clone());
                        let status = build.status.unwrap_or_default();
                        // Only reload in-place when the user is still in the
                        // destination directory and not mid-navigation to
                        // somewhere else (which would cancel their navigation).
                        let nav_target = self
                            .directory_runtime
                            .pending_load
                            .as_ref()
                            .map(|l| l.target_cwd.as_path());
                        let nav_to_dest = nav_target == Some(dest_dir.as_path());
                        if dest_dir == self.cwd && (nav_target.is_none() || nav_to_dest) {
                            let _ = self.queue_directory_load(PendingDirectoryLoad {
                                token: 0,
                                target_cwd: dest_dir,
                                previous_cwd: self.cwd.clone(),
                                previous_selected_path: None,
                                previous_selection_name: None,
                                reselect_path: None,
                                history_mode: DirectoryHistoryMode::None,
                                refresh_search: false,
                                completion: DirectoryLoadCompletion::Status(status),
                            });
                        } else {
                            // User navigated away — just surface the status.
                            // Navigation will load dest_dir fresh if they return.
                            self.status = status;
                        }
                    } else if let Some(prog) = &mut self.paste_progress {
                        prog.completed = build.completed;
                    }
                    dirty = true;
                }
                JobResult::Trash(build) => {
                    if build.token != self.trash_token {
                        continue;
                    }
                    if build.done {
                        self.trash_progress = None;
                        let source_cwd = self
                            .trash_source_cwd
                            .take()
                            .unwrap_or_else(|| self.cwd.clone());
                        let status = build.status.unwrap_or_default();
                        // Only reload in-place when the user is still in the
                        // source directory and not mid-navigation to somewhere
                        // else (which would cancel their navigation).
                        let nav_target = self
                            .directory_runtime
                            .pending_load
                            .as_ref()
                            .map(|l| l.target_cwd.as_path());
                        let nav_to_source = nav_target == Some(source_cwd.as_path());
                        if source_cwd == self.cwd && (nav_target.is_none() || nav_to_source) {
                            let _ = self.queue_directory_load(PendingDirectoryLoad {
                                token: 0,
                                target_cwd: source_cwd,
                                previous_cwd: self.cwd.clone(),
                                previous_selected_path: None,
                                previous_selection_name: None,
                                reselect_path: None,
                                history_mode: DirectoryHistoryMode::None,
                                refresh_search: false,
                                completion: DirectoryLoadCompletion::Status(status),
                            });
                        } else {
                            // User navigated away — just surface the status.
                            // Navigation will load source_cwd fresh if they return.
                            self.status = status;
                        }
                    } else if let Some(prog) = &mut self.trash_progress {
                        prog.completed = build.completed;
                    }
                    dirty = true;
                }
                JobResult::Restore(build) => {
                    if build.token != self.restore_token {
                        continue;
                    }
                    if build.done {
                        self.restore_progress = None;
                        let source_cwd = self
                            .restore_source_cwd
                            .take()
                            .unwrap_or_else(|| self.cwd.clone());
                        let status = build.status.unwrap_or_default();
                        let nav_target = self
                            .directory_runtime
                            .pending_load
                            .as_ref()
                            .map(|l| l.target_cwd.as_path());
                        let nav_to_source = nav_target == Some(source_cwd.as_path());
                        if source_cwd == self.cwd && (nav_target.is_none() || nav_to_source) {
                            let _ = self.queue_directory_load(PendingDirectoryLoad {
                                token: 0,
                                target_cwd: source_cwd,
                                previous_cwd: self.cwd.clone(),
                                previous_selected_path: None,
                                previous_selection_name: None,
                                reselect_path: None,
                                history_mode: DirectoryHistoryMode::None,
                                refresh_search: false,
                                completion: DirectoryLoadCompletion::Status(status),
                            });
                        } else {
                            self.status = status;
                        }
                    } else if let Some(prog) = &mut self.restore_progress {
                        prog.completed = build.completed;
                    }
                    dirty = true;
                }
                JobResult::Preview(build) => {
                    self.cache_preview_result_with_code_line_limit(
                        &build.entry,
                        &build.variant,
                        build.code_line_limit,
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
                        .selected_entry()
                        .map(|entry| {
                            entry.path == build.entry.path
                                && entry.modified == build.entry.modified
                                && entry.size == build.entry.size
                        })
                        .unwrap_or(false);
                    let is_current_variant =
                        build.variant == self.current_preview_request_options();
                    if build.token != self.preview_state.token
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
                                &self.preview_state.load_state,
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
                            self.preview_state.metrics.stale_results_dropped += 1;
                            continue;
                        }
                    }

                    self.preview_state.content = build.result;
                    self.preview_state.load_state = None;
                    self.apply_current_comic_preview_metadata();
                    self.apply_current_epub_preview_metadata();
                    self.sync_current_preview_line_count();
                    self.preview_state.scroll = 0;
                    self.preview_state.horizontal_scroll = 0;
                    self.sync_preview_scroll();
                    if build_visual_kind.is_some() {
                        self.refresh_static_image_preloads();
                    }
                    if build_is_comic || build_is_epub_section || is_current_entry {
                        self.prefetch_nearby_audio_previews();
                        self.schedule_preview_prefetch();
                    }
                    self.preview_state.metrics.applied_results += 1;
                    dirty = true;
                }
            }
        }

        if (processed == JOB_RESULT_APPLY_MAX_PER_TICK
            || (processed > 0 && started_at.elapsed() >= JOB_RESULT_APPLY_TIME_BUDGET))
            && let Ok(job) = self.scheduler.try_recv()
        {
            self.scheduler.defer_result(job);
        }

        dirty
    }
}

#[cfg(test)]
mod tests;
