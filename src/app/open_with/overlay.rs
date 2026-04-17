use std::path::Path;

use super::super::{
    App,
    state::{OpenWithApp, OpenWithOverlay, OpenWithRow},
};
use crate::fs::{detached_open_command, open_in_system};
use anyhow::Result;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum FallbackOpenOutcome {
    DefaultApp,
    #[cfg(target_os = "macos")]
    TextEditor,
}

// ── Read-only accessors ───────────────────────────────────────────────────────

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

// ── Overlay control and launch logic ─────────────────────────────────────────

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
        let entry = entry.clone();
        let path = entry.path.clone();

        let apps = super::discovery::discover_open_with_apps_for_entry(&entry);
        self.handle_discovered_open_with_apps(&path, apps, open_with_fallback, |app| {
            detached_open_command(&app.program, &app.args)
        });
    }

    pub(super) fn confirm_open_with_index(&mut self, index: usize) -> Result<()> {
        let Some(row) = self
            .overlays
            .open_with
            .as_ref()
            .and_then(|overlay| overlay.rows.get(index))
        else {
            return Ok(());
        };
        let display_name = row.app.display_name.clone();
        let program = row.app.program.clone();
        let args = row.app.args.clone();
        let requires_terminal = row.app.requires_terminal;

        self.overlays.open_with = None;

        if requires_terminal {
            self.pending_terminal_command = Some((program, args));
            self.status.clear();
        } else {
            match detached_open_command(&program, &args) {
                Ok(()) => self.status.clear(),
                Err(_) => self.status = format!("Failed to open with {display_name}"),
            }
        }

        Ok(())
    }

    /// Dispatches a discovered app list: falls back to the system opener for
    /// zero apps, launches directly for one, and opens the overlay for two or
    /// more.
    ///
    /// `launch_app` is called only for GUI apps (`requires_terminal == false`).
    /// Terminal apps set `pending_terminal_command` on `self` directly so that
    /// the caller in `lib.rs` can suspend the TUI before running them.
    pub(in crate::app) fn handle_discovered_open_with_apps<F, G>(
        &mut self,
        path: &Path,
        mut apps: Vec<OpenWithApp>,
        mut fallback_open: F,
        mut launch_app: G,
    ) where
        F: FnMut(&Path) -> std::result::Result<FallbackOpenOutcome, String>,
        G: FnMut(&OpenWithApp) -> std::io::Result<()>,
    {
        match apps.len() {
            0 => match fallback_open(path) {
                Ok(FallbackOpenOutcome::DefaultApp) => {
                    self.status = "No apps found, opened with default".to_string();
                }
                #[cfg(target_os = "macos")]
                Ok(FallbackOpenOutcome::TextEditor) => {
                    self.status = "No apps found, opened in text editor".to_string();
                }
                Err(e) => self.status = format!("Failed to open: {e}"),
            },
            1 => {
                let app = apps.remove(0);
                if app.requires_terminal {
                    self.pending_terminal_command = Some((app.program.clone(), app.args.clone()));
                    self.status.clear();
                } else {
                    match launch_app(&app) {
                        Ok(()) => self.status.clear(),
                        Err(_) => self.status = format!("Failed to open with {}", app.display_name),
                    }
                }
            }
            _ => {
                self.overlays.help = false;
                self.overlays.open_with = Some(build_open_with_overlay(apps));
                self.status.clear();
            }
        }
    }
}

fn build_open_with_overlay(apps: Vec<OpenWithApp>) -> OpenWithOverlay {
    let rows = apps
        .into_iter()
        .enumerate()
        .filter_map(|(index, app)| {
            let shortcut = assign_shortcut(index)?;
            let mut label = app.display_name.clone();
            if app.requires_terminal {
                label.push_str(" (terminal)");
            }
            if app.is_default {
                label.push_str(" (default)");
            }
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

fn open_with_fallback(path: &Path) -> std::result::Result<FallbackOpenOutcome, String> {
    #[cfg(target_os = "macos")]
    {
        if super::path_is_text_like(path) {
            return open_in_text_editor(path).map(|()| FallbackOpenOutcome::TextEditor);
        }
    }

    open_in_system(path).map(|()| FallbackOpenOutcome::DefaultApp)
}

#[cfg(target_os = "macos")]
fn open_in_text_editor(path: &Path) -> std::result::Result<(), String> {
    use std::process::{Command, Stdio};

    let status = Command::new("open")
        .arg("-t")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("open: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("open exited with {status}"))
    }
}

// ── Test seam ─────────────────────────────────────────────────────────────────

#[cfg(test)]
impl App {
    /// Injects a single-row open-with overlay pointing at the given command.
    /// Used only in tests to exercise the confirm/launch path without real discovery.
    pub(crate) fn inject_open_with_for_test(
        &mut self,
        display_name: &str,
        program: &str,
        args: Vec<String>,
        requires_terminal: bool,
    ) {
        use super::super::state::{OpenWithApp, OpenWithOverlay, OpenWithRow};
        self.overlays.open_with = Some(OpenWithOverlay {
            title: "Open With".to_string(),
            rows: vec![OpenWithRow {
                shortcut: '1',
                label: display_name.to_string(),
                app: OpenWithApp {
                    display_name: display_name.to_string(),
                    desktop_id: None,
                    program: program.to_string(),
                    args,
                    is_default: false,
                    requires_terminal,
                },
            }],
        });
    }
}
