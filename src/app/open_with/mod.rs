mod discovery;

use super::{
    App,
    state::{OpenWithApp, OpenWithOverlay, OpenWithRow},
};
use crate::fs::rect_contains;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

impl App {
    pub fn open_with_is_open(&self) -> bool {
        self.overlays.open_with.is_some()
    }

    pub fn open_with_title(&self) -> &str {
        self.overlays
            .open_with
            .as_ref()
            .map(|overlay| overlay.title.as_str())
            .unwrap_or("")
    }

    pub fn open_with_row_count(&self) -> usize {
        self.overlays
            .open_with
            .as_ref()
            .map(|overlay| overlay.rows.len())
            .unwrap_or(0)
    }

    pub fn open_with_row_label(&self, index: usize) -> &str {
        self.overlays
            .open_with
            .as_ref()
            .and_then(|overlay| overlay.rows.get(index))
            .map(|row| row.label.as_str())
            .unwrap_or("")
    }

    pub fn open_with_row_shortcut(&self, index: usize) -> Option<char> {
        self.overlays
            .open_with
            .as_ref()
            .and_then(|overlay| overlay.rows.get(index))
            .map(|row| row.shortcut)
    }
}

impl App {
    pub(in crate::app) fn open_open_with_overlay(&mut self) {
        let Some(entry) = self.selected_entry() else {
            self.status = "Nothing selected".to_string();
            return;
        };
        if entry.is_dir() {
            self.status = "Open With is for files".to_string();
            return;
        }
        let path = entry.path.clone();

        let apps = discovery::discover_open_with_apps(&path);
        if apps.is_empty() {
            self.status = "No applications found".to_string();
            return;
        }

        self.overlays.help = false;
        self.overlays.open_with = Some(build_open_with_overlay(apps));
        self.status.clear();
    }

    pub(in crate::app) fn handle_open_with_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.overlays.open_with = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.overlays.open_with = None;
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(index) = self.open_with_row_index_for_shortcut(ch) {
                    self.confirm_open_with_index(index)?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub(in crate::app) fn handle_open_with_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let inside = self
                .input
                .frame_state
                .open_with_panel
                .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
            if !inside {
                self.overlays.open_with = None;
                return Ok(());
            }

            if let Some(hit) = self
                .input
                .frame_state
                .open_with_hits
                .iter()
                .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                .cloned()
            {
                self.confirm_open_with_index(hit.index)?;
            }
        }

        Ok(())
    }

    fn open_with_row_index_for_shortcut(&self, ch: char) -> Option<usize> {
        let needle = ch.to_ascii_lowercase();
        self.overlays.open_with.as_ref().and_then(|overlay| {
            overlay
                .rows
                .iter()
                .position(|row| row.shortcut.to_ascii_lowercase() == needle)
        })
    }

    fn confirm_open_with_index(&mut self, index: usize) -> Result<()> {
        let Some(display_name) = self
            .overlays
            .open_with
            .as_ref()
            .and_then(|overlay| overlay.rows.get(index))
            .map(|row| row.app.display_name.clone())
        else {
            return Ok(());
        };

        self.overlays.open_with = None;
        self.status = format!("Would open with {display_name}");
        Ok(())
    }
}

fn build_open_with_overlay(apps: Vec<OpenWithApp>) -> OpenWithOverlay {
    let rows = apps
        .into_iter()
        .enumerate()
        .filter_map(|(index, app)| {
            let shortcut = assign_shortcut(index)?;
            let label = app.display_name.clone();
            Some(OpenWithRow {
                shortcut,
                label,
                app,
            })
        })
        .collect();

    OpenWithOverlay {
        title: "Open With".to_string(),
        rows,
    }
}

/// Assigns a keyboard shortcut for the row at `index`.
/// Slots 0–8 → `'1'`–`'9'`, slots 9–34 → `'a'`–`'z'`.
fn assign_shortcut(index: usize) -> Option<char> {
    if index < 9 {
        char::from_digit((index + 1) as u32, 10)
    } else if index < 9 + 26 {
        Some((b'a' + (index - 9) as u8) as char)
    } else {
        None
    }
}
