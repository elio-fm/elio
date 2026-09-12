use super::super::App;
use crate::background_jobs::job_results::{
    DirectoryBuild, DirectoryFingerprintBuild, DirectoryItemCountBuild, DuplicateScanBatchBuild,
    DuplicateScanBuild, GitStatusBuild, SearchBatchBuild, SearchBuild,
};
use crate::fuzzy_finder::SearchCache;
use std::sync::Arc;

impl App {
    pub(super) fn apply_directory_job_result(&mut self, build: DirectoryBuild) -> bool {
        let Some(load) = self.file_browser.directory_runtime.pending_load.clone() else {
            return false;
        };
        if build.token != self.file_browser.directory_runtime.load_token
            || build.token != load.token
            || build.cwd != load.target_cwd
        {
            return false;
        }

        self.file_browser.directory_runtime.pending_load = None;

        match build.result {
            Ok(snapshot) => self.apply_directory_snapshot(load, snapshot),
            Err(error) => {
                self.preview.exit_fullscreen_after_directory_load = false;
                self.exit_fullscreen_preview();
                self.status = format!("Cannot open {}: {}", build.cwd.display(), error);
            }
        }
        true
    }

    pub(super) fn apply_directory_fingerprint_job_result(
        &mut self,
        build: DirectoryFingerprintBuild,
    ) -> bool {
        let Some(scan) = self
            .file_browser
            .directory_runtime
            .pending_fingerprint_scan
            .clone()
        else {
            return false;
        };
        if build.token != self.file_browser.directory_runtime.fingerprint_token
            || build.token != scan.token
            || build.cwd != scan.cwd
            || build.show_hidden != scan.show_hidden
        {
            return false;
        }

        self.file_browser.directory_runtime.pending_fingerprint_scan = None;

        let Ok(fingerprint) = build.result else {
            return false;
        };
        if self.file_browser.directory_runtime.pending_load.is_some()
            || fingerprint == self.file_browser.directory_runtime.fingerprint
        {
            return false;
        }
        self.queue_directory_reload(true).is_ok()
    }

    pub(super) fn apply_directory_item_count_job_result(
        &mut self,
        build: DirectoryItemCountBuild,
    ) -> bool {
        self.cache_directory_item_count(
            build.path.clone(),
            build.modified,
            build.show_hidden,
            build.item_count,
        );
        self.should_redraw_for_directory_item_count(&build.path, build.modified, build.show_hidden)
    }

    pub(super) fn apply_git_status_job_result(&mut self, build: GitStatusBuild) -> bool {
        self.file_browser
            .apply_git_status(build.token, build.cwd, build.branch, build.dirty)
    }

    pub(super) fn apply_search_batch_job_result(&mut self, build: SearchBatchBuild) -> bool {
        if build.token != self.fuzzy_finder.token
            || build.cwd != self.file_browser.cwd
            || build.show_hidden != self.effective_show_hidden()
            || build.fingerprint != self.file_browser.directory_runtime.fingerprint
        {
            return false;
        }

        let mut dirty = false;
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
        dirty
    }

    pub(super) fn apply_search_job_result(&mut self, build: SearchBuild) -> bool {
        if build.token != self.fuzzy_finder.token
            || build.cwd != self.file_browser.cwd
            || build.show_hidden != self.effective_show_hidden()
            || build.fingerprint != self.file_browser.directory_runtime.fingerprint
        {
            return false;
        }

        self.fuzzy_finder.loading = false;

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
        true
    }

    pub(super) fn apply_duplicate_scan_batch_job_result(
        &mut self,
        build: DuplicateScanBatchBuild,
    ) -> bool {
        if build.token != self.duplicate_finder.scan_token
            || build.cwd != self.file_browser.cwd
            || build.show_hidden != self.effective_show_hidden()
        {
            return false;
        }
        self.apply_duplicate_batch(build.batch);
        true
    }

    pub(super) fn apply_duplicate_scan_job_result(&mut self, build: DuplicateScanBuild) -> bool {
        if build.token != self.duplicate_finder.scan_token
            || build.cwd != self.file_browser.cwd
            || build.show_hidden != self.effective_show_hidden()
        {
            return false;
        }
        self.apply_duplicate_result(build.result);
        true
    }
}
