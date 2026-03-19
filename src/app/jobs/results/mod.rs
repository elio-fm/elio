use super::*;
use std::{collections::HashMap, sync::Arc, time::{Duration, Instant}};

const JOB_RESULT_APPLY_MAX_PER_TICK: usize = 12;
const JOB_RESULT_APPLY_TIME_BUDGET: Duration = Duration::from_millis(2);

impl App {
    fn refresh_static_image_preloads_for_cached_selected_page_preview(
        &mut self,
        build_has_page_image: bool,
        is_current_entry: bool,
    ) {
        if build_has_page_image && is_current_entry {
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
                JobResult::Preview(build) => {
                    self.cache_preview_result(&build.entry, &build.variant, &build.result);
                    let build_is_comic = build.result.kind == preview::PreviewKind::Comic;
                    let build_is_epub_section = matches!(
                        build.variant,
                        preview::PreviewRequestOptions::EpubSection(_)
                    );
                    let build_has_page_image =
                        build.result.preview_visual.as_ref().is_some_and(|visual| {
                            visual.kind == preview::PreviewVisualKind::PageImage
                        });
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
                    {
                        self.refresh_static_image_preloads_for_cached_selected_page_preview(
                            build_has_page_image,
                            is_current_entry,
                        );
                        self.preview_state.metrics.stale_results_dropped += 1;
                        continue;
                    }

                    self.preview_state.content = build.result;
                    self.preview_state.load_state = None;
                    self.apply_current_comic_preview_metadata();
                    self.apply_current_epub_preview_metadata();
                    self.sync_current_preview_line_count();
                    self.preview_state.scroll = 0;
                    self.preview_state.horizontal_scroll = 0;
                    self.sync_preview_scroll();
                    if build_has_page_image {
                        self.refresh_static_image_preloads();
                    }
                    if build_is_comic || build_is_epub_section || is_current_entry {
                        self.schedule_preview_prefetch();
                    }
                    self.preview_state.metrics.applied_results += 1;
                    dirty = true;
                }
            }
        }

        if processed == JOB_RESULT_APPLY_MAX_PER_TICK
            || (processed > 0 && started_at.elapsed() >= JOB_RESULT_APPLY_TIME_BUDGET)
        {
            if let Ok(job) = self.scheduler.try_recv() {
                self.scheduler.defer_result(job);
            }
        }

        dirty
    }
}

#[cfg(test)]
mod tests;
