use super::super::App;
use crate::filesystem::SortMode;

#[cfg(test)]
#[path = "tests/folder_sizes.rs"]
mod tests;

impl App {
    pub(crate) fn cancel_folder_sizes(&mut self) {
        let token = self.job_scheduler.folder_sizes.replace(Vec::new());
        self.file_browser.folder_sizes.begin(token, &[]);
    }

    pub(crate) fn start_folder_sizes(&mut self) {
        self.cancel_folder_sizes();
        if self.file_browser.sort_mode != SortMode::Size {
            return;
        }
        // Directory symlinks stay unknown: never recursively traverse their targets.
        let paths = self
            .file_browser
            .unfiltered_entries
            .iter()
            .filter(|entry| entry.is_dir() && entry.symlink.is_none())
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();
        let token = self.job_scheduler.folder_sizes.replace(paths.clone());
        self.file_browser.folder_sizes.begin(token, &paths);
        self.sort_folder_sizes();
    }

    fn sort_folder_sizes(&mut self) {
        crate::filesystem::sort_entries_by_recursive_size(
            &mut self.file_browser.unfiltered_entries,
            crate::config::ui().folders_first,
            &self.file_browser.folder_sizes.current,
        );
    }

    pub(crate) fn process_folder_sizes(&mut self) -> bool {
        let results = self.job_scheduler.folder_sizes.drain();
        self.apply_folder_size_results(results)
    }

    fn apply_folder_size_results(
        &mut self,
        results: Vec<crate::background_jobs::folder_sizes::FolderSizeResult>,
    ) -> bool {
        if self.file_browser.sort_mode != SortMode::Size {
            return false;
        }
        let mut changed = false;
        for result in results {
            changed |= self
                .file_browser
                .folder_sizes
                .apply(result.token, result.path, result.size);
        }
        if changed {
            self.sort_folder_sizes();
            self.file_browser.apply_local_filter_preserving_selection();
            self.sync_scroll();
            self.remember_current_directory_view();
            self.file_browser.directory_count_viewport = None;
        }
        changed
    }
}
