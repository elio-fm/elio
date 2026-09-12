use std::path::Path;

use super::super::{App, state::PendingTerminalTask};
#[cfg(any(test, target_os = "macos", not(unix)))]
use crate::opening::open_in_system;
use crate::opening::{
    launch_application,
    open_with::{self, OpenWithApplication},
};
use anyhow::Result;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum FallbackOpenOutcome {
    #[cfg_attr(all(unix, not(target_os = "macos"), not(test)), allow(dead_code))]
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
            .and_then(|row| row.shortcut)
    }

    pub fn open_with_selected_index(&self) -> usize {
        self.overlays
            .open_with
            .as_ref()
            .map(|overlay| overlay.selected)
            .unwrap_or(0)
    }
}

// ── Overlay control and launch logic ─────────────────────────────────────────

impl App {
    pub(in crate::app) fn open_open_with_overlay(&mut self) {
        let Some(entry) = self.selected_entry() else {
            self.status = "Nothing selected".to_string();
            return;
        };
        self.open_open_with_overlay_for_entry(entry.clone());
    }

    pub(in crate::app) fn open_open_with_overlay_for_entry(&mut self, entry: crate::fs::Entry) {
        let path = entry.path.clone();
        #[cfg(test)]
        let apps = open_with::applications_for_test()
            .unwrap_or_else(|| open_with::applications_for(&entry));
        #[cfg(not(test))]
        let apps = open_with::applications_for(&entry);
        self.handle_discovered_open_with_apps(&path, apps, open_with_fallback, |app| {
            launch_application(&app.program, &app.args)
        });
    }

    pub(in crate::app) fn confirm_open_with_index(&mut self, index: usize) -> Result<()> {
        let Some(application) = self
            .overlays
            .open_with
            .as_ref()
            .and_then(|selection| selection.application_at(index))
        else {
            return Ok(());
        };
        let display_name = application.display_name.clone();
        let program = application.program.clone();
        let args = application.args.clone();
        let requires_terminal = application.requires_terminal;

        self.overlays.open_with = None;

        if requires_terminal {
            self.pending_terminal_task = Some(PendingTerminalTask::Command { program, args });
            self.status.clear();
        } else {
            match launch_application(&program, &args) {
                Ok(()) => self.status.clear(),
                Err(error) => {
                    self.status = format!("Failed to open with {display_name}: {error}");
                }
            }
        }

        Ok(())
    }

    /// Dispatches a discovered app list: falls back to the system opener for
    /// zero apps, launches directly for one, and opens the overlay for two or
    /// more.
    ///
    /// `launch_app` is called only for GUI apps (`requires_terminal == false`).
    /// Terminal apps set `pending_terminal_task` on `self` directly so that
    /// the caller in `lib.rs` can suspend the TUI before running them.
    pub(in crate::app) fn handle_discovered_open_with_apps<F, G>(
        &mut self,
        path: &Path,
        mut apps: Vec<OpenWithApplication>,
        mut fallback_open: F,
        mut launch_app: G,
    ) where
        F: FnMut(&Path) -> std::result::Result<FallbackOpenOutcome, String>,
        G: FnMut(&OpenWithApplication) -> std::io::Result<()>,
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
                Err(e) if e == "No apps found" => self.status = e,
                Err(e) => self.status = format!("Failed to open: {e}"),
            },
            1 => {
                let app = apps.remove(0);
                if app.requires_terminal {
                    self.pending_terminal_task = Some(PendingTerminalTask::Command {
                        program: app.program.clone(),
                        args: app.args.clone(),
                    });
                    self.status.clear();
                } else {
                    match launch_app(&app) {
                        Ok(()) => self.status = format!("Opened with {}", app.display_name),
                        Err(error) => {
                            self.status =
                                format!("Failed to open with {}: {error}", app.display_name);
                        }
                    }
                }
            }
            _ => {
                self.overlays.help = false;
                self.overlays.open_with = Some(open_with::ApplicationSelection::new(
                    apps,
                    &crate::config::key_bindings().open_with_reserved_shortcuts(),
                ));
                self.status.clear();
            }
        }
    }

    pub(in crate::app) fn move_open_with_selection(&mut self, delta: isize) {
        if let Some(selection) = self.overlays.open_with.as_mut() {
            selection.move_selection(delta);
        }
    }

    pub(in crate::app) fn confirm_selected_open_with_row(&mut self) -> Result<()> {
        let Some(index) = self
            .overlays
            .open_with
            .as_ref()
            .map(|overlay| overlay.selected)
        else {
            return Ok(());
        };

        self.confirm_open_with_index(index)
    }
}

fn open_with_fallback(path: &Path) -> std::result::Result<FallbackOpenOutcome, String> {
    #[cfg(target_os = "macos")]
    {
        if open_with::is_editor_compatible(path) {
            return crate::opening::open_in_text_editor(path)
                .map(|()| FallbackOpenOutcome::TextEditor);
        }
        return open_in_system(path).map(|()| FallbackOpenOutcome::DefaultApp);
    }

    #[cfg(all(unix, not(target_os = "macos"), not(test)))]
    {
        let _ = path;
        Err("No apps found".to_string())
    }

    #[cfg(all(any(not(unix), test), not(target_os = "macos")))]
    open_in_system(path).map(|()| FallbackOpenOutcome::DefaultApp)
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
        self.inject_open_with_rows_for_test(vec![(
            display_name.to_string(),
            program.to_string(),
            args,
            requires_terminal,
        )]);
    }

    pub(crate) fn inject_open_with_rows_for_test(
        &mut self,
        rows: Vec<(String, String, Vec<String>, bool)>,
    ) {
        self.overlays.open_with = Some(open_with::ApplicationSelection::from_test_rows(rows));
    }
}
