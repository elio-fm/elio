use super::*;
use std::{path::Path, time::Instant};

impl App {
    pub(crate) fn directory_item_count_label(&self, entry: &Entry) -> Option<String> {
        self.directory_item_count_value(entry)
            .map(format_item_count)
    }

    pub(crate) fn directory_item_count_value(&self, entry: &Entry) -> Option<usize> {
        self.file_browser
            .directory_item_count(entry, self.effective_show_hidden())
    }

    pub(super) fn cache_directory_item_count(
        &mut self,
        path: PathBuf,
        modified: Option<SystemTime>,
        show_hidden: bool,
        item_count: Option<usize>,
    ) {
        self.file_browser
            .cache_directory_item_count(path, modified, show_hidden, item_count);
    }

    pub(super) fn queue_visible_directory_item_counts(&mut self) {
        self.file_browser.update_directory_count_viewport(
            self.input.frame_state.metrics.cols,
            self.input.frame_state.metrics.rows_visible,
            self.effective_show_hidden(),
            Instant::now() + DIRECTORY_ITEM_COUNT_IDLE_DELAY,
        );
    }

    pub(crate) fn process_directory_item_count_timer(&mut self) -> bool {
        if !self
            .file_browser
            .directory_count_timer_ready(Instant::now())
        {
            return false;
        }
        self.submit_visible_directory_item_counts();
        false
    }

    pub(crate) fn pending_directory_item_count_timer(&self) -> Option<std::time::Duration> {
        self.file_browser
            .pending_directory_count_timer(Instant::now())
    }

    fn submit_visible_directory_item_counts(&mut self) {
        let requests = self
            .visible_entry_indices()
            .into_iter()
            .filter_map(|index| {
                self.file_browser.entries.get(index).and_then(|entry| {
                    entry.is_dir().then_some((
                        index.abs_diff(self.file_browser.selected),
                        index,
                        entry,
                    ))
                })
            })
            .collect::<Vec<_>>();
        let mut requests = requests
            .into_iter()
            .filter_map(|(distance, index, entry)| {
                self.directory_item_count_request_for(entry)
                    .map(|request| (distance, index, request))
            })
            .collect::<Vec<_>>();
        requests.sort_by_key(|(distance, index, _)| (*distance, *index));

        for (_, _, request) in requests {
            let _ = self.jobs.scheduler.submit_directory_item_count(request);
        }
    }

    pub(super) fn should_redraw_for_directory_item_count(
        &self,
        path: &Path,
        modified: Option<SystemTime>,
        show_hidden: bool,
    ) -> bool {
        self.file_browser.should_redraw_for_directory_item_count(
            path,
            modified,
            show_hidden,
            self.effective_show_hidden(),
            self.input.frame_state.metrics.cols,
            self.input.frame_state.metrics.rows_visible,
        )
    }

    fn directory_item_count_request_for(
        &self,
        entry: &Entry,
    ) -> Option<jobs::DirectoryItemCountRequest> {
        let key = crate::file_browser::FileBrowserState::directory_item_count_key(
            entry,
            self.effective_show_hidden(),
        )?;
        if self.file_browser.directory_item_count_is_cached(&key) {
            return None;
        }
        Some(jobs::DirectoryItemCountRequest {
            path: key.path,
            modified: key.modified,
            show_hidden: key.show_hidden,
        })
    }

    pub(super) fn visible_entry_indices(&self) -> Vec<usize> {
        self.file_browser.visible_entry_indices(
            self.input.frame_state.metrics.cols,
            self.input.frame_state.metrics.rows_visible,
        )
    }
}
