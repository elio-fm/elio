use super::super::{
    App,
    state::{NewTabTemplate, PendingNewTabCommand},
};
use crate::{
    background_jobs::{
        job_requests::{DestinationCommandRecipient, DestinationCommandRequest},
        job_results::DestinationCommandBuild,
    },
    config::{CloseLastTabBehavior, NewTabDestination, TabDestination},
    file_browser::{FileBrowserState, MAX_TABS},
};
use anyhow::Result;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

const NEW_TAB_COMMAND_STATUS_DELAY: Duration = Duration::from_millis(300);

struct NewTabPath {
    cwd: PathBuf,
    focus: Option<PathBuf>,
    in_trash: bool,
}

impl App {
    pub(crate) fn new_tab(&mut self) -> Result<()> {
        if self.chooser_mode() {
            return Ok(());
        }
        if !self.tabs.can_open() {
            self.status = format!("Maximum of {MAX_TABS} tabs reached");
            return Ok(());
        }
        if self.pending_new_tab_command.is_some() {
            return Ok(());
        }

        let template = self.new_tab_template();
        let destination = crate::config::tabs().new_tab_destination.clone();
        match destination {
            Some(destination) => self.open_configured_new_tab(destination, template),
            None => {
                let destination =
                    NewTabPath::active_directory(template.cwd.clone(), template.in_trash);
                self.open_new_tab_at(template, destination)
            }
        }
    }

    fn new_tab_template(&self) -> NewTabTemplate {
        NewTabTemplate {
            cwd: self.file_browser.cwd.clone(),
            start_in_grid: self.file_browser.view_mode == crate::file_browser::ViewMode::Grid,
            zoom_level: self.file_browser.zoom_level,
            sort_mode: self.file_browser.sort_mode,
            show_hidden: self.file_browser.show_hidden,
            in_trash: self.file_browser.in_trash,
        }
    }

    fn open_configured_new_tab(
        &mut self,
        destination: NewTabDestination,
        template: NewTabTemplate,
    ) -> Result<()> {
        self.open_new_tab_destination(destination.destination, destination.fallback, template)
    }

    fn open_new_tab_destination(
        &mut self,
        destination: TabDestination,
        fallback: Option<TabDestination>,
        template: NewTabTemplate,
    ) -> Result<()> {
        match destination {
            TabDestination::Path(path) => match NewTabPath::from_path(path) {
                Some(destination) => self.open_new_tab_at(template, destination),
                None => self.open_new_tab_fallback(fallback, template),
            },
            TabDestination::Command(command) => {
                self.destination_command_token = self.destination_command_token.wrapping_add(1);
                let token = self.destination_command_token;
                let cwd = template.cwd.clone();
                self.pending_new_tab_command = Some(PendingNewTabCommand {
                    token,
                    fallback,
                    template,
                    submitted_at: Instant::now(),
                    status_shown: false,
                });
                self.status.clear();
                if self
                    .job_scheduler
                    .submit_destination_command(DestinationCommandRequest {
                        token,
                        recipient: DestinationCommandRecipient::NewTab,
                        label: "New tab".to_string(),
                        command,
                        cwd,
                    })
                {
                    Ok(())
                } else {
                    let pending = self
                        .pending_new_tab_command
                        .take()
                        .expect("pending tab command was just stored");
                    self.open_new_tab_fallback(pending.fallback, pending.template)
                }
            }
        }
    }

    fn open_new_tab_fallback(
        &mut self,
        fallback: Option<TabDestination>,
        template: NewTabTemplate,
    ) -> Result<()> {
        match fallback {
            Some(destination) => self.open_new_tab_destination(destination, None, template),
            None => {
                let destination =
                    NewTabPath::active_directory(template.cwd.clone(), template.in_trash);
                self.open_new_tab_at(template, destination)
            }
        }
    }

    pub(crate) fn apply_new_tab_command_job_result(
        &mut self,
        build: DestinationCommandBuild,
    ) -> bool {
        if self
            .pending_new_tab_command
            .as_ref()
            .is_none_or(|pending| pending.token != build.token)
        {
            return false;
        }
        let pending = self
            .pending_new_tab_command
            .take()
            .expect("matching pending tab command was checked");
        if !self.tabs.can_open() {
            self.status = format!("Maximum of {MAX_TABS} tabs reached");
            return true;
        }

        let result = match build.result {
            Ok(path) => self.open_new_tab_at(
                pending.template,
                NewTabPath {
                    in_trash: crate::places::path_is_trash(&path),
                    cwd: path,
                    focus: None,
                },
            ),
            Err(_) => self.open_new_tab_fallback(pending.fallback, pending.template),
        };
        if let Err(error) = result {
            self.status = error.to_string();
            self.pending_missing_tab_target = None;
        } else if self.pending_new_tab_command.is_none()
            && let Err(error) = self.continue_missing_tab_creation()
        {
            self.status = error.to_string();
            self.pending_missing_tab_target = None;
        }
        true
    }

    pub(crate) fn show_pending_new_tab_command_status(&mut self, now: Instant) -> bool {
        let should_show = self
            .pending_new_tab_command
            .as_mut()
            .is_some_and(|pending| {
                if pending.status_shown
                    || now.saturating_duration_since(pending.submitted_at)
                        < NEW_TAB_COMMAND_STATUS_DELAY
                {
                    return false;
                }
                pending.status_shown = true;
                true
            });
        if should_show {
            self.status = "Opening a new tab".to_string();
        }
        should_show
    }

    fn open_new_tab_at(&mut self, template: NewTabTemplate, destination: NewTabPath) -> Result<()> {
        self.store_active_paged_preview_state();
        self.invalidate_active_tab_work();
        let mut browser = FileBrowserState::new(
            destination.cwd,
            template.start_in_grid,
            template.zoom_level,
            template.show_hidden,
        );
        browser.sort_mode = template.sort_mode;
        browser.in_trash = destination.in_trash;
        self.tabs.open_after_active(&mut self.file_browser, browser);
        self.paged_preview_tabs
            .insert(self.tabs.active_index(), Default::default());
        debug_assert_eq!(self.tabs.len(), self.paged_preview_tabs.len());
        self.activate_current_tab()?;
        if let Some(path) = destination.focus
            && let Some(load) = self.file_browser.directory_runtime.pending_load.as_mut()
        {
            load.reselect_path = Some(path);
        }
        self.status.clear();
        Ok(())
    }

    pub(crate) fn close_tab(&mut self, index: usize) -> Result<()> {
        if self.chooser_mode() || index >= self.tabs.len() {
            return Ok(());
        }
        if self.tabs.len() == 1 {
            match crate::config::tabs().close_last_tab {
                CloseLastTabBehavior::KeepOpen => {
                    self.status = "Cannot close the last tab".to_string();
                }
                CloseLastTabBehavior::Quit => self.quit(),
                CloseLastTabBehavior::QuitWithoutCd => self.quit_without_cd(),
            }
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
        if self.chooser_mode() {
            return Ok(());
        }
        if index >= self.tabs.len() && crate::config::tabs().switch_tab_creates_missing {
            if self.pending_new_tab_command.is_some() {
                return Ok(());
            }
            self.pending_missing_tab_target = Some(index);
            self.continue_missing_tab_creation()?;
        }
        if index >= self.tabs.len() || self.tabs.is_active(index) {
            return Ok(());
        }
        self.store_active_paged_preview_state();
        self.invalidate_active_tab_work();
        if self.tabs.activate(&mut self.file_browser, index) {
            self.activate_current_tab()?;
            self.status.clear();
        }
        Ok(())
    }

    fn continue_missing_tab_creation(&mut self) -> Result<()> {
        while let Some(target) = self.pending_missing_tab_target {
            if self.tabs.len() > target || !self.tabs.can_open() {
                self.pending_missing_tab_target = None;
                break;
            }
            self.new_tab()?;
            if self.pending_new_tab_command.is_some() {
                break;
            }
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
            self.status.clear();
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

impl NewTabPath {
    fn active_directory(cwd: PathBuf, in_trash: bool) -> Self {
        Self {
            cwd,
            focus: None,
            in_trash,
        }
    }

    fn from_path(path: PathBuf) -> Option<Self> {
        let metadata = std::fs::metadata(&path).ok()?;
        if metadata.is_dir() {
            return Some(Self {
                in_trash: crate::places::path_is_trash(&path),
                cwd: path,
                focus: None,
            });
        }
        let cwd = path.parent()?.to_path_buf();
        cwd.is_dir().then_some(Self {
            in_trash: crate::places::path_is_trash(&cwd),
            cwd,
            focus: Some(path),
        })
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
    fn new_tab_command_status_waits_three_hundred_milliseconds() {
        let root = temp_root();
        let mut app = App::new_at(root.clone()).expect("app should load temporary root");
        let submitted_at = Instant::now();
        app.pending_new_tab_command = Some(PendingNewTabCommand {
            token: 1,
            fallback: None,
            template: app.new_tab_template(),
            submitted_at,
            status_shown: false,
        });

        assert!(!app.show_pending_new_tab_command_status(
            submitted_at + NEW_TAB_COMMAND_STATUS_DELAY - Duration::from_millis(1)
        ));
        assert!(app.status.is_empty());
        assert!(
            app.show_pending_new_tab_command_status(submitted_at + NEW_TAB_COMMAND_STATUS_DELAY)
        );
        assert_eq!(app.status, "Opening a new tab");
        assert!(!app.show_pending_new_tab_command_status(
            submitted_at + NEW_TAB_COMMAND_STATUS_DELAY + Duration::from_millis(1)
        ));

        app.file_browser.directory_runtime.watch = None;
        drop(app);
        fs::remove_dir_all(root).expect("temporary root should be removed");
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
