use super::*;
use std::{collections::HashMap, sync::Arc};

impl App {
    pub fn process_background_jobs(&mut self) -> bool {
        let mut dirty = false;

        while let Ok(job) = self.scheduler.try_recv() {
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
                    if build.token != self.search_token || build.cwd != self.cwd {
                        continue;
                    }

                    self.search_loading = false;
                    dirty = true;

                    match build.result {
                        Ok(candidates) => {
                            self.search_cache = Some(SearchCache {
                                cwd: build.cwd,
                                scope: build.scope,
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
                    self.cache_preview_result(&build.entry, &build.result);
                    let is_current_entry = self
                        .selected_entry()
                        .map(|entry| {
                            entry.path == build.entry.path
                                && entry.modified == build.entry.modified
                                && entry.size == build.entry.size
                        })
                        .unwrap_or(false);
                    if build.token != self.preview_state.token || !is_current_entry {
                        self.preview_state.metrics.stale_results_dropped += 1;
                        continue;
                    }

                    self.preview_state.content = build.result;
                    self.preview_state.load_state = None;
                    self.preview_state.scroll = 0;
                    self.preview_state.horizontal_scroll = 0;
                    self.sync_preview_scroll();
                    self.preview_state.metrics.applied_results += 1;
                    dirty = true;
                }
            }
        }

        dirty
    }
}

#[cfg(test)]
mod tests;
