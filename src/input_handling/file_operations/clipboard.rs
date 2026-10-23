use crate::app::App;
use crate::file_operations::ClipOp;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::path::PathBuf;

impl App {
    pub(crate) fn yank(&mut self) {
        self.set_clipboard(ClipOp::Yank);
    }

    pub(crate) fn cut(&mut self) {
        self.set_clipboard(ClipOp::Cut);
    }

    fn set_clipboard(&mut self, op: ClipOp) {
        let paths = self.file_browser.selected_or_focused_paths_sorted();
        if self.file_operations.set_clipboard(paths, op) {
            self.file_browser.selected_paths.clear();
            self.status.clear();
        }
    }

    pub(crate) fn paste(&mut self) -> Result<()> {
        let preparation = self.file_operations.prepare_paste(&self.file_browser.cwd);
        if let Some(request) = preparation.request {
            self.job_scheduler.submit_paste(request);
        }
        if let Some(status) = preparation.status {
            self.status = status;
        }
        Ok(())
    }

    pub(crate) fn paste_system_file_clipboard(&mut self) -> Result<()> {
        if let Some(clipboard) = crate::system_clipboard::read_files() {
            self.prepare_system_clipboard_paths(
                clipboard.paths,
                match clipboard.operation {
                    crate::system_clipboard::FileClipboardOperation::Copy => ClipOp::Yank,
                    crate::system_clipboard::FileClipboardOperation::Cut => ClipOp::Cut,
                },
            )?;
        }
        Ok(())
    }

    #[cfg(any(unix, test))]
    pub(crate) fn drop_external_paths(&mut self, paths: Vec<PathBuf>, op: ClipOp) -> Result<bool> {
        self.prepare_drag_and_drop_paths(paths, op)
    }

    #[cfg(any(unix, test))]
    fn prepare_system_clipboard_paths(&mut self, paths: Vec<PathBuf>, op: ClipOp) -> Result<bool> {
        let preparation =
            self.file_operations
                .prepare_system_clipboard_paste(&self.file_browser.cwd, paths, op);
        if let Some(request) = preparation.request {
            self.job_scheduler.submit_paste(request);
        }
        // Progress is rendered by the left status-bar chip. Do not leave a
        // stale right-side message competing with it while the transfer runs.
        self.status = preparation.status.unwrap_or_default();
        Ok(preparation.accepted)
    }

    #[cfg(any(unix, test))]
    fn prepare_drag_and_drop_paths(&mut self, paths: Vec<PathBuf>, op: ClipOp) -> Result<bool> {
        let preparation =
            self.file_operations
                .prepare_drag_and_drop(&self.file_browser.cwd, paths, op);
        if let Some(request) = preparation.request {
            self.job_scheduler.submit_paste(request);
        }
        // Drag-and-drop uses the same progress chip, not a second running
        // message in the right-side status area.
        self.status = preparation.status.unwrap_or_default();
        Ok(preparation.accepted)
    }

    pub(crate) fn link_yanked(&mut self, relative: bool) -> Result<()> {
        let completion = self
            .file_operations
            .create_symlinks(&self.file_browser.cwd, relative)?;
        if completion.created {
            let _ = self.queue_directory_reload(false);
        }
        self.status = completion.status;
        Ok(())
    }

    pub(crate) fn open_copy_overlay(&mut self) {
        let paths = self.file_browser.selected_or_focused_paths_sorted();
        self.open_copy_overlay_for_paths(paths);
    }

    pub(crate) fn copy_items_to_system_clipboard(&mut self) {
        let paths = self.file_browser.selected_or_focused_paths_sorted();
        self.copy_paths_to_system_clipboard(paths);
    }

    pub(crate) fn copy_paths_to_system_clipboard(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            self.status = "Nothing to copy".to_string();
            return;
        }
        self.status = self.file_operations.copy_items_to_system_clipboard(&paths);
    }

    pub(crate) fn open_copy_overlay_for_paths(&mut self, paths: Vec<PathBuf>) {
        if !self
            .file_operations
            .open_copy_overlay(&self.file_browser.cwd, &paths)
        {
            self.status = "Nothing to copy".to_string();
            return;
        }
        self.overlays.help = false;
        self.status.clear();
    }

    fn confirm_copy_index(&mut self, index: usize) {
        if let Some(status) = self.file_operations.confirm_copy_index(index) {
            self.status = status;
        }
    }

    pub(crate) fn handle_copy_key(&mut self, key: KeyEvent) -> Result<()> {
        if crate::config::key_bindings().matches_cancel_key(key) {
            self.file_operations.dismiss_copy();
            return Ok(());
        }

        match key.code {
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(index) = self.copy_row_index_for_shortcut(ch) {
                    self.confirm_copy_index(index);
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub(crate) fn handle_copy_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let inside = self
                .input
                .screen_regions
                .copy_panel
                .is_some_and(|panel| panel.contains((mouse.column, mouse.row).into()));
            if !inside {
                self.file_operations.dismiss_copy();
                return Ok(());
            }

            if let Some(hit) = self
                .input
                .screen_regions
                .copy_hits
                .iter()
                .find(|hit| hit.rect.contains((mouse.column, mouse.row).into()))
                .cloned()
            {
                self.confirm_copy_index(hit.index);
            }
        }

        Ok(())
    }

    fn copy_row_index_for_shortcut(&self, ch: char) -> Option<usize> {
        let needle = ch.to_ascii_lowercase();
        self.file_operations.copy_overlay().and_then(|overlay| {
            overlay
                .rows
                .iter()
                .position(|row| row.shortcut.to_ascii_lowercase() == needle)
        })
    }
}
