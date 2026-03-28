use super::super::{
    App, SidebarItemKind,
    state::{GoToDestination, GoToOverlay, GoToOverlayRow},
};
use crate::fs::{rect_contains, trash_dir};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::{env, path::PathBuf};

impl App {
    pub fn goto_is_open(&self) -> bool {
        self.goto_overlay.is_some()
    }

    pub fn goto_title(&self) -> &str {
        self.goto_overlay
            .as_ref()
            .map(|overlay| overlay.title.as_str())
            .unwrap_or("")
    }

    pub fn goto_row_count(&self) -> usize {
        self.goto_overlay
            .as_ref()
            .map(|overlay| overlay.rows.len())
            .unwrap_or(0)
    }

    pub fn goto_row_label(&self, index: usize) -> &str {
        self.goto_overlay
            .as_ref()
            .and_then(|overlay| overlay.rows.get(index))
            .map(|row| row.label.as_str())
            .unwrap_or("")
    }

    pub fn goto_row_shortcut(&self, index: usize) -> Option<char> {
        self.goto_overlay
            .as_ref()
            .and_then(|overlay| overlay.rows.get(index))
            .map(|row| row.shortcut)
    }
}

impl App {
    pub(in crate::app) fn open_goto_overlay(&mut self) {
        self.help_open = false;
        self.goto_overlay = Some(build_goto_overlay(self));
        self.status.clear();
    }

    pub(in crate::app) fn handle_goto_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.goto_overlay = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.goto_overlay = None;
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(index) = self.goto_row_index_for_shortcut(ch) {
                    self.confirm_goto_index(index)?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub(in crate::app) fn handle_goto_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let inside = self
                .frame_state
                .goto_panel
                .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
            if !inside {
                self.goto_overlay = None;
                return Ok(());
            }

            if let Some(hit) = self
                .frame_state
                .goto_hits
                .iter()
                .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                .cloned()
            {
                self.confirm_goto_index(hit.index)?;
            }
        }

        Ok(())
    }

    fn goto_row_index_for_shortcut(&self, ch: char) -> Option<usize> {
        let needle = ch.to_ascii_lowercase();
        self.goto_overlay.as_ref().and_then(|overlay| {
            overlay
                .rows
                .iter()
                .position(|row| row.shortcut.to_ascii_lowercase() == needle)
        })
    }

    fn confirm_goto_index(&mut self, index: usize) -> Result<()> {
        let Some(destination) = self
            .goto_overlay
            .as_ref()
            .and_then(|overlay| overlay.rows.get(index).map(|row| row.destination.clone()))
        else {
            return Ok(());
        };

        match destination {
            GoToDestination::Top => {
                self.goto_overlay = None;
                self.select_index(0);
            }
            GoToDestination::Path(path) => {
                self.goto_overlay = None;
                self.set_dir(path)?;
            }
            GoToDestination::Missing(status) => {
                self.status = status;
            }
        }

        Ok(())
    }
}

fn build_goto_overlay(app: &App) -> GoToOverlay {
    let rows = vec![
        build_goto_row('g', "top", GoToDestination::Top),
        build_goto_row(
            'd',
            "downloads",
            downloads_destination(app)
                .map(GoToDestination::Path)
                .unwrap_or_else(|| GoToDestination::Missing("Downloads not available".to_string())),
        ),
        build_goto_row(
            'h',
            "home",
            home_directory()
                .map(GoToDestination::Path)
                .unwrap_or_else(|| GoToDestination::Missing("Home not available".to_string())),
        ),
        build_goto_row(
            'c',
            ".config",
            config_directory()
                .map(GoToDestination::Path)
                .unwrap_or_else(|| GoToDestination::Missing(".config not available".to_string())),
        ),
        build_goto_row(
            't',
            "trash",
            trash_destination(app)
                .map(GoToDestination::Path)
                .unwrap_or_else(|| GoToDestination::Missing("Trash not available".to_string())),
        ),
    ];

    GoToOverlay {
        title: "Go to".to_string(),
        rows,
    }
}

fn build_goto_row(shortcut: char, label: &str, destination: GoToDestination) -> GoToOverlayRow {
    GoToOverlayRow {
        shortcut,
        label: label.to_string(),
        destination,
    }
}

fn home_directory() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn downloads_destination(app: &App) -> Option<PathBuf> {
    app.sidebar
        .iter()
        .filter_map(|row| row.item())
        .find(|item| item.kind == SidebarItemKind::Downloads)
        .map(|item| item.path.clone())
        .or_else(|| home_directory().map(|home| home.join("Downloads")))
        .filter(|path| path.exists())
}

fn config_directory() -> Option<PathBuf> {
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(config_home));
    }

    home_directory().map(|home| home.join(".config"))
}

fn trash_destination(app: &App) -> Option<PathBuf> {
    app.sidebar
        .iter()
        .filter_map(|row| row.item())
        .find(|item| item.kind == SidebarItemKind::Trash)
        .map(|item| item.path.clone())
        .or_else(|| home_directory().and_then(|home| trash_dir(&home)))
}
