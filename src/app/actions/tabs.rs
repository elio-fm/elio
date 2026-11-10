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

        self.store_active_paged_preview_state();
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
        self.paged_preview_tabs
            .insert(self.tabs.active_index(), Default::default());
        debug_assert_eq!(self.tabs.len(), self.paged_preview_tabs.len());
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
            self.store_active_paged_preview_state();
            self.invalidate_active_tab_work();
        }
        let active_changed = self.tabs.close(&mut self.file_browser, index);
        self.paged_preview_tabs.remove(index);
        debug_assert_eq!(self.tabs.len(), self.paged_preview_tabs.len());
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
        self.store_active_paged_preview_state();
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
        self.store_active_paged_preview_state();
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
        self.restore_active_paged_preview_state();
        self.clear_wheel_scroll();
        self.clear_preview_directory_stats();
        self.reset_directory_watch();
        self.queue_directory_reload(false)?;
        self.refresh_preview();
        Ok(())
    }

    fn store_active_paged_preview_state(&mut self) {
        debug_assert_eq!(self.tabs.len(), self.paged_preview_tabs.len());
        let active = self.tabs.active_index();
        let state = &mut self.paged_preview_tabs[active];
        state.comic_session = self.preview.comic.session.take();
        state.epub_session = self.preview.epub.session.take();
        state.pdf_session = self.preview.pdf.session.take();
    }

    fn restore_active_paged_preview_state(&mut self) {
        debug_assert_eq!(self.tabs.len(), self.paged_preview_tabs.len());
        let active = self.tabs.active_index();
        let state = std::mem::take(&mut self.paged_preview_tabs[active]);
        self.preview.comic.session = state.comic_session;
        self.preview.epub.session = state.epub_session;
        self.preview.pdf.session = state.pdf_session;
        self.preview.comic.displayed_page_source = None;
        self.preview.pdf.activation_ready_at = None;
        self.preview.pdf.last_navigation_direction = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        filesystem::{Entry, EntryKind},
        terminal_images::ImageProtocol,
    };
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_root() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("elio-tab-preview-state-{unique}"));
        fs::create_dir_all(&root).expect("temporary root should be created");
        root.canonicalize().unwrap_or(root)
    }

    fn selected_pdf_entry(root: &std::path::Path) -> Entry {
        let path = root.join("book.pdf");
        fs::write(&path, b"%PDF-1.7\n").expect("PDF fixture should be written");
        let metadata = fs::metadata(&path).expect("PDF fixture metadata should be readable");
        Entry {
            path,
            name: "book.pdf".to_string(),
            name_key: "book.pdf".to_string(),
            kind: EntryKind::File,
            symlink: None,
            size: metadata.len(),
            modified: metadata.modified().ok(),
            readonly: false,
        }
    }

    fn select_pdf(app: &mut App, entry: Entry) {
        app.file_browser.entries = vec![entry];
        app.file_browser.selected = 0;
        app.sync_pdf_preview_selection();
    }

    fn set_pdf_page(app: &mut App, page: usize) {
        app.preview
            .pdf
            .session
            .as_mut()
            .expect("PDF session should be active")
            .current_page = page;
    }

    fn assert_pdf_page(app: &App, page: usize) {
        assert_eq!(
            app.preview
                .pdf
                .session
                .as_ref()
                .map(|session| session.current_page),
            Some(page)
        );
    }

    #[test]
    fn pdf_page_position_is_isolated_by_tab_actions() {
        let root = temp_root();
        let mut app = App::new_at(root.clone()).expect("app should load temporary root");
        app.preview.terminal_images.protocol = ImageProtocol::KittyGraphics;
        app.preview.pdf.pdf_tools_available = true;
        let entry = selected_pdf_entry(&root);

        select_pdf(&mut app, entry.clone());
        set_pdf_page(&mut app, 2);
        app.new_tab().expect("second tab should open");

        select_pdf(&mut app, entry);
        set_pdf_page(&mut app, 17);
        app.activate_tab(0).expect("first tab should activate");
        assert_pdf_page(&app, 2);

        app.activate_tab(1).expect("second tab should activate");
        assert_pdf_page(&app, 17);

        app.close_tab(1).expect("second tab should close");
        assert_pdf_page(&app, 2);

        app.file_browser.directory_runtime.watch = None;
        drop(app);
        fs::remove_dir_all(root).expect("temporary root should be removed");
    }
}
