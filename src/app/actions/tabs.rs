use super::super::App;
use crate::file_browser::{FileBrowserState, MAX_TABS};
use anyhow::Result;

impl App {
    pub(crate) fn new_tab(&mut self) -> Result<()> {
        if self.chooser_mode() {
            return Ok(());
        }
        if !self.tabs.can_open() {
            self.status = format!("Maximum of {MAX_TABS} tabs reached");
            return Ok(());
        }

        self.invalidate_active_tab_work();
        let mut browser = FileBrowserState::new(
            self.file_browser.cwd.clone(),
            self.file_browser.view_mode == crate::file_browser::ViewMode::Grid,
            self.file_browser.zoom_level,
            self.file_browser.show_hidden,
        );
        browser.sort_mode = self.file_browser.sort_mode;
        browser.in_trash = self.file_browser.in_trash;
        self.tabs.open_after_active(&mut self.file_browser, browser);
        self.activate_current_tab()?;
        self.status = format!("Opened tab {}", self.tabs.active_index() + 1);
        Ok(())
    }

    pub(crate) fn close_tab(&mut self, index: usize) -> Result<()> {
        if self.chooser_mode() || index >= self.tabs.len() {
            return Ok(());
        }
        if self.tabs.len() == 1 {
            self.status = "Cannot close the only tab".to_string();
            return Ok(());
        }

        let closes_active = self.tabs.is_active(index);
        if closes_active {
            self.invalidate_active_tab_work();
        }
        let active_changed = self.tabs.close(&mut self.file_browser, index);
        if active_changed {
            self.activate_current_tab()?;
        }
        self.status.clear();
        Ok(())
    }

    pub(crate) fn activate_tab(&mut self, index: usize) -> Result<()> {
        if self.chooser_mode() || index >= self.tabs.len() || self.tabs.is_active(index) {
            return Ok(());
        }
        self.invalidate_active_tab_work();
        if self.tabs.activate(&mut self.file_browser, index) {
            self.activate_current_tab()?;
            self.status = format!("Switched to tab {}", index + 1);
        }
        Ok(())
    }

    pub(crate) fn cycle_tabs(&mut self, delta: isize) -> Result<()> {
        if self.chooser_mode() || self.tabs.len() < 2 {
            return Ok(());
        }
        self.invalidate_active_tab_work();
        if self.tabs.cycle(&mut self.file_browser, delta) {
            self.activate_current_tab()?;
            self.status = format!("Switched to tab {}", self.tabs.active_index() + 1);
        }
        Ok(())
    }

    fn invalidate_active_tab_work(&mut self) {
        self.file_browser.invalidate_pending_background_work();
        self.job_scheduler.cancel_directory_fingerprints();
        self.job_scheduler.cancel_directory_stats();
        self.cancel_folder_sizes();
        self.job_scheduler.cancel_search();
        self.fuzzy_finder.token = self.fuzzy_finder.token.wrapping_add(1);
    }

    fn activate_current_tab(&mut self) -> Result<()> {
        self.clear_wheel_scroll();
        self.clear_preview_directory_stats();
        self.reset_directory_watch();
        self.queue_directory_reload(false)?;
        self.refresh_preview();
        Ok(())
    }
}
