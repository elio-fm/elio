use super::super::{App, SearchScope};
use crate::background_jobs::job_requests::SearchRequest;
use crate::file_browser::{DirectoryHistoryMode, DirectoryLoadCompletion};
use crate::fuzzy_finder::SearchRow;
use anyhow::Result;
use std::{ffi::OsStr, path::PathBuf, sync::Arc};

impl App {
    pub fn search_is_open(&self) -> bool {
        self.fuzzy_finder.search.is_some()
    }

    pub fn search_query(&self) -> &str {
        self.fuzzy_finder
            .search
            .as_ref()
            .map(|search| search.query.as_str())
            .unwrap_or("")
    }

    pub fn search_match_count(&self) -> usize {
        self.fuzzy_finder
            .search
            .as_ref()
            .map_or(0, crate::fuzzy_finder::SearchState::match_count)
    }

    #[cfg(test)]
    pub fn search_candidate_count(&self) -> usize {
        self.fuzzy_finder
            .search
            .as_ref()
            .map_or(0, crate::fuzzy_finder::SearchState::candidate_count)
    }

    pub fn search_scanned_count(&self) -> usize {
        self.fuzzy_finder
            .search
            .as_ref()
            .map_or(0, crate::fuzzy_finder::SearchState::scanned_count)
    }

    pub fn search_index_is_limited(&self) -> bool {
        self.fuzzy_finder
            .search
            .as_ref()
            .is_some_and(|search| search.stats.is_limited())
    }

    pub fn search_scope(&self) -> Option<SearchScope> {
        self.fuzzy_finder.search.as_ref().map(|search| search.scope)
    }

    pub fn search_is_loading(&self) -> bool {
        self.fuzzy_finder
            .search
            .as_ref()
            .is_some_and(|search| search.loading)
    }

    pub fn search_error(&self) -> Option<&str> {
        self.fuzzy_finder
            .search
            .as_ref()
            .and_then(|search| search.error.as_deref())
    }

    pub fn search_rows(&self, max_rows: usize) -> Vec<SearchRow> {
        self.fuzzy_finder
            .search
            .as_ref()
            .map(|search| search.rows(max_rows))
            .unwrap_or_default()
    }

    pub fn search_scroll_top(&self) -> usize {
        self.fuzzy_finder
            .search
            .as_ref()
            .map_or(0, |search| search.scroll)
    }

    pub fn search_query_cursor(&self) -> usize {
        self.fuzzy_finder
            .search
            .as_ref()
            .map_or(0, crate::fuzzy_finder::SearchState::query_cursor)
    }

    pub(crate) fn open_fuzzy_finder(&mut self, scope: SearchScope) -> Result<()> {
        self.clear_wheel_scroll();
        self.overlays.help = false;
        let show_hidden = self.effective_show_hidden();
        let cached = self
            .fuzzy_finder
            .cache
            .as_ref()
            .filter(|cache| {
                cache.cwd == self.file_browser.cwd
                    && cache.scope == scope
                    && cache.show_hidden == show_hidden
                    && cache.fingerprint == self.file_browser.directory_runtime.fingerprint
            })
            .map(|cache| (cache.candidates.clone(), cache.stats));
        let (candidates, stats) = cached.clone().unwrap_or_else(|| {
            (
                Arc::new(Vec::new()),
                crate::fuzzy_finder::SearchIndexStats::default(),
            )
        });
        let loading = cached.is_none();
        if loading {
            self.prewarm_search_index(scope);
        }
        self.fuzzy_finder.search = Some(crate::fuzzy_finder::SearchState::new(
            scope, candidates, stats, loading,
        ));
        self.status.clear();
        Ok(())
    }

    pub(crate) fn prewarm_search_index(&mut self, scope: SearchScope) {
        self.fuzzy_finder.token = self.fuzzy_finder.token.wrapping_add(1);
        self.fuzzy_finder.loading = true;
        self.fuzzy_finder.cache = None;
        let request = SearchRequest {
            token: self.fuzzy_finder.token,
            cwd: self.file_browser.cwd.clone(),
            scope,
            show_hidden: self.effective_show_hidden(),
            fingerprint: self.file_browser.directory_runtime.fingerprint,
        };
        if !self.job_scheduler.submit_search(request) {
            self.fuzzy_finder.loading = false;
            if let Some(search) = &mut self.fuzzy_finder.search
                && search.scope == scope
            {
                search.loading = false;
                search.error = Some("Search worker unavailable".to_string());
            }
        }
    }

    pub(crate) fn close_search_overlay(&mut self) {
        self.fuzzy_finder.search = None;
        self.fuzzy_finder.loading = false;
        self.fuzzy_finder.token = self.fuzzy_finder.token.wrapping_add(1);
        self.job_scheduler.cancel_search();
        self.clear_wheel_scroll();
    }

    pub(crate) fn move_search_selection(&mut self, delta: isize) {
        if let Some(search) = &mut self.fuzzy_finder.search {
            search.move_selection(delta);
        }
        self.sync_search_scroll();
    }

    pub(crate) fn refresh_search_matches(&mut self, previous_query: &str) {
        if let Some(search) = &mut self.fuzzy_finder.search {
            search.refresh_matches(previous_query);
        }
        self.sync_search_scroll();
    }

    pub(crate) fn select_search_index(&mut self, index: usize) {
        if let Some(search) = &mut self.fuzzy_finder.search {
            search.select_index(index);
        }
        self.sync_search_scroll();
    }

    pub(crate) fn confirm_search_selection(&mut self) -> Result<()> {
        let Some(path) = self
            .fuzzy_finder
            .search
            .as_ref()
            .and_then(crate::fuzzy_finder::SearchState::selected_path)
        else {
            return Ok(());
        };
        self.reveal_path(path)?;
        self.close_search_overlay();
        Ok(())
    }

    pub(crate) fn sync_search_scroll(&mut self) -> bool {
        let rows_visible = self.input.screen_regions.search_rows_visible;
        self.fuzzy_finder
            .search
            .as_mut()
            .is_some_and(|search| search.sync_scroll(rows_visible))
    }

    pub(crate) fn reveal_path(&mut self, path: PathBuf) -> Result<()> {
        if path.is_dir() {
            return self.set_dir_transition(
                path,
                DirectoryHistoryMode::PushCurrent,
                None,
                DirectoryLoadCompletion::Status("Opened folder from search".to_string()),
            );
        }
        let Some(parent) = path.parent() else {
            return Ok(());
        };
        let file_name = path
            .file_name()
            .and_then(OsStr::to_str)
            .map(str::to_string)
            .unwrap_or_default();
        self.set_dir_transition(
            parent.to_path_buf(),
            DirectoryHistoryMode::PushCurrent,
            Some(path),
            DirectoryLoadCompletion::Status(format!("Located {file_name}")),
        )
    }
}
