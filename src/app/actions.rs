use super::*;
use anyhow::{Result, bail};
use std::{env, path::PathBuf};

impl App {
    pub fn reload(&mut self) -> Result<()> {
        let previous_name = self.selected_entry().map(|entry| entry.name.clone());
        self.entries = support::read_entries(&self.cwd, self.show_hidden)?;
        support::sort_entries(&mut self.entries, self.sort_mode);
        self.sidebar = support::build_sidebar_items();

        self.selected = match previous_name {
            Some(name) => self
                .entries
                .iter()
                .position(|entry| entry.name == name)
                .unwrap_or(0),
            None => 0,
        };
        self.clamp_selection();
        self.sync_scroll();
        self.refresh_preview();
        self.clear_wheel_scroll();
        Ok(())
    }

    pub fn preview_lines(&self, max_lines: usize) -> Vec<Line<'static>> {
        self.preview_cache
            .iter()
            .take(max_lines)
            .cloned()
            .collect::<Vec<_>>()
    }

    pub(super) fn refresh_preview(&mut self) {
        self.preview_cache = match self.selected_entry() {
            Some(entry) => support::build_preview(entry),
            None => vec![Line::from("No selection")],
        };
    }

    pub fn selection_summary(&self) -> String {
        match self.selected_entry() {
            Some(entry) => format!(
                "{} of {} selected  •  {}  •  {}",
                self.selected.saturating_add(1),
                self.entries.len(),
                entry.kind_label(),
                entry.name,
            ),
            None => format!("0 items  •  {}", self.cwd.display()),
        }
    }

    pub fn status_message(&self) -> &str {
        &self.status
    }

    pub fn can_go_back(&self) -> bool {
        !self.back_history.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_history.is_empty()
    }

    pub(super) fn set_selected(&mut self, index: usize) {
        let next = index.min(self.entries.len().saturating_sub(1));
        if next != self.selected {
            self.selected = next;
            self.refresh_preview();
        } else {
            self.selected = next;
        }
        self.sync_scroll();
    }

    pub(super) fn set_selected_last(&mut self) {
        if !self.entries.is_empty() {
            let last = self.entries.len() - 1;
            self.set_selected(last);
        }
    }

    pub(super) fn set_selected_delta(&mut self, delta: isize) {
        if self.entries.is_empty() {
            self.selected = 0;
            self.preview_cache = vec![Line::from("No selection")];
            return;
        }

        let max_index = self.entries.len().saturating_sub(1) as isize;
        let next = (self.selected as isize + delta).clamp(0, max_index) as usize;
        self.set_selected(next);
    }

    pub(super) fn page(&mut self, direction: isize) {
        let rows = self.frame_state.metrics.rows_visible.max(1) as isize;
        if self.view_mode == ViewMode::Grid {
            self.move_grid_vertical(direction * rows);
        } else {
            self.set_selected_delta(direction * rows);
        }
    }

    pub(super) fn move_vertical(&mut self, rows: isize) {
        if self.view_mode == ViewMode::Grid {
            self.move_grid_vertical(rows);
        } else {
            self.set_selected_delta(rows);
        }
    }

    pub(super) fn move_by(&mut self, delta: isize) {
        self.set_selected_delta(delta);
    }

    pub(super) fn move_grid_vertical(&mut self, rows: isize) {
        if self.entries.is_empty() {
            self.selected = 0;
            return;
        }

        let cols = self.frame_state.metrics.cols.max(1);
        let current_row = self.selected / cols;
        let current_col = self.selected % cols;
        let total_rows = self.entries.len().div_ceil(cols);
        let target_row = current_row as isize + rows;

        if target_row < 0 || target_row >= total_rows as isize {
            return;
        }

        let target_index = target_row as usize * cols + current_col;
        if target_index >= self.entries.len() {
            return;
        }

        self.set_selected(target_index);
    }

    pub(super) fn adjust_zoom(&mut self, delta: i8) {
        let next = (self.zoom_level as i8 + delta).clamp(0, 2) as u8;
        if next == self.zoom_level {
            self.status = format!("Directory zoom limit: {}", self.zoom_level);
            return;
        }
        self.zoom_level = next;
        self.status = format!("Directory zoom set to {}", self.zoom_level);
        self.sync_scroll();
    }

    pub(super) fn reset_zoom(&mut self) {
        self.zoom_level = 1;
        self.status = format!("Directory zoom reset to {}", self.zoom_level);
        self.sync_scroll();
    }

    pub(super) fn select_index(&mut self, index: usize) {
        self.set_selected(index);
    }

    pub(super) fn select_last(&mut self) {
        self.set_selected_last();
    }

    pub(super) fn clamp_selection(&mut self) {
        if self.entries.is_empty() {
            self.selected = 0;
            self.scroll_row = 0;
            self.preview_cache = vec![Line::from("No selection")];
        } else if self.selected >= self.entries.len() {
            self.selected = self.entries.len() - 1;
        }
    }

    pub(super) fn sync_scroll(&mut self) -> bool {
        let previous = self.scroll_row;
        if self.entries.is_empty() {
            self.scroll_row = 0;
            return previous != self.scroll_row;
        }

        let cols = self.frame_state.metrics.cols.max(1);
        let rows_visible = self.frame_state.metrics.rows_visible.max(1);
        let selected_row = self.selected / cols;
        if selected_row < self.scroll_row {
            self.scroll_row = selected_row;
        } else if selected_row >= self.scroll_row + rows_visible {
            self.scroll_row = selected_row + 1 - rows_visible;
        }
        self.scroll_row = self.scroll_row.min(self.max_scroll_row());
        previous != self.scroll_row
    }

    fn max_scroll_row(&self) -> usize {
        if self.entries.is_empty() {
            return 0;
        }

        let cols = self.frame_state.metrics.cols.max(1);
        let rows_visible = self.frame_state.metrics.rows_visible.max(1);
        let total_rows = self.entries.len().div_ceil(cols);
        total_rows.saturating_sub(rows_visible)
    }

    pub(super) fn set_dir(&mut self, path: PathBuf) -> Result<()> {
        self.set_dir_with_history(path, true)
    }

    pub(super) fn set_dir_with_history(&mut self, path: PathBuf, record_history: bool) -> Result<()> {
        if !path.is_dir() {
            bail!("{} is not a directory", path.display());
        }
        let normalized = path.canonicalize().unwrap_or(path);
        if normalized == self.cwd {
            self.status = format!("Already in {}", self.cwd.display());
            return Ok(());
        }
        if record_history {
            self.back_history.push(self.cwd.clone());
            self.forward_history.clear();
        }
        self.cwd = normalized;
        self.selected = 0;
        self.scroll_row = 0;
        self.reload()?;
        self.status.clear();
        Ok(())
    }

    pub(super) fn go_parent(&mut self) -> Result<()> {
        let Some(parent) = self.cwd.parent() else {
            self.status = "Already at filesystem root".to_string();
            return Ok(());
        };
        self.set_dir(parent.to_path_buf())
    }

    pub(super) fn go_home(&mut self) -> Result<()> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"));
        self.set_dir(home)
    }

    pub(super) fn go_back(&mut self) -> Result<()> {
        let Some(previous) = self.back_history.pop() else {
            self.status = "No previous folder".to_string();
            return Ok(());
        };
        self.forward_history.push(self.cwd.clone());
        self.set_dir_with_history(previous, false)
    }

    pub(super) fn go_forward(&mut self) -> Result<()> {
        let Some(next) = self.forward_history.pop() else {
            self.status = "No next folder".to_string();
            return Ok(());
        };
        self.back_history.push(self.cwd.clone());
        self.set_dir_with_history(next, false)
    }

    pub(super) fn open_in_system(&mut self) -> Result<()> {
        let Some(entry) = self.selected_entry() else {
            return Ok(());
        };

        let target = entry.path.clone();
        for (program, args) in [("gio", vec!["open"]), ("xdg-open", Vec::new())] {
            match support::detached_open(program, &args, &target) {
                Ok(()) => {
                    self.status = format!("Opened {}", target.display());
                    return Ok(());
                }
                Err(error) => {
                    self.status = format!(
                        "Failed to open {} with {}: {}",
                        target.display(),
                        program,
                        error
                    );
                }
            }
        }
        Ok(())
    }
}
