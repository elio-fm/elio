mod discovery;

use std::path::Path;

use super::{
    App,
    state::{OpenWithApp, OpenWithOverlay, OpenWithRow},
};
use crate::fs::{detached_open_command, open_in_system, rect_contains};
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
        self.handle_discovered_open_with_apps(&path, apps, open_in_system, |app| {
            detached_open_command(&app.program, &app.args)
        });
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
        F: FnMut(&Path) -> std::result::Result<(), String>,
        G: FnMut(&OpenWithApp) -> std::io::Result<()>,
    {
        match apps.len() {
            0 => match fallback_open(path) {
                Ok(()) => self.status = "No apps found, opened with default".to_string(),
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
            let label = if app.is_default {
                format!("{} (default)", app.display_name)
            } else {
                app.display_name.clone()
            };
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

#[cfg(test)]
impl App {
    /// Injects a single-row open-with overlay pointing at the given command.
    /// Used only in tests to exercise the confirm/launch path without real discovery.
    pub(in crate::app) fn inject_open_with_for_test(
        &mut self,
        display_name: &str,
        program: &str,
        args: Vec<String>,
        requires_terminal: bool,
    ) {
        use super::state::{OpenWithApp, OpenWithOverlay, OpenWithRow};
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_dir_path(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-open-with-{label}-{unique}"))
    }

    fn fake_open_with_app(display_name: &str) -> OpenWithApp {
        OpenWithApp {
            display_name: display_name.to_string(),
            desktop_id: None,
            program: "fake".to_string(),
            args: vec!["--arg".to_string()],
            is_default: true,
            requires_terminal: false,
        }
    }

    fn fake_terminal_app(display_name: &str) -> OpenWithApp {
        OpenWithApp {
            display_name: display_name.to_string(),
            desktop_id: None,
            program: "nvim".to_string(),
            args: vec!["/tmp/file.txt".to_string()],
            is_default: false,
            requires_terminal: true,
        }
    }

    #[test]
    fn zero_discovered_apps_fall_back_to_default_open() {
        let root = temp_dir_path("fallback-root");
        fs::create_dir_all(&root).expect("create temp root");
        let path = root.join("file.txt");
        fs::write(&path, "hello").expect("write temp file");

        let fallback_called = Cell::new(false);
        let mut app = App::new_at(root.clone()).expect("create app");
        app.handle_discovered_open_with_apps(
            &path,
            vec![],
            |_| {
                fallback_called.set(true);
                Ok(())
            },
            |_| unreachable!("launch should not be called when no apps were discovered"),
        );

        assert!(fallback_called.get(), "fallback opener must be called");
        assert!(
            app.overlays.open_with.is_none(),
            "overlay must remain closed"
        );
        assert_eq!(app.status, "No apps found, opened with default");

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn single_discovered_app_launches_without_opening_overlay() {
        let root = temp_dir_path("single-launch-root");
        fs::create_dir_all(&root).expect("create temp root");
        let path = root.join("file.txt");
        fs::write(&path, "hello").expect("write temp file");

        let launched = RefCell::new(None::<String>);
        let mut app = App::new_at(root.clone()).expect("create app");
        app.handle_discovered_open_with_apps(
            &path,
            vec![fake_open_with_app("Fake App")],
            |_| unreachable!("fallback should not be called when one app was discovered"),
            |app| {
                *launched.borrow_mut() = Some(app.display_name.clone());
                Ok(())
            },
        );

        assert_eq!(launched.into_inner().as_deref(), Some("Fake App"));
        assert!(
            app.overlays.open_with.is_none(),
            "overlay must remain closed"
        );
        assert!(
            app.status.is_empty(),
            "successful launch should clear status"
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn single_discovered_app_launch_failure_sets_status_without_overlay() {
        let root = temp_dir_path("single-launch-fail-root");
        fs::create_dir_all(&root).expect("create temp root");
        let path = root.join("file.txt");
        fs::write(&path, "hello").expect("write temp file");

        let mut app = App::new_at(root.clone()).expect("create app");
        app.handle_discovered_open_with_apps(
            &path,
            vec![fake_open_with_app("Ghost App")],
            |_| unreachable!("fallback should not be called when one app was discovered"),
            |_| Err(std::io::Error::new(std::io::ErrorKind::NotFound, "missing")),
        );

        assert!(
            app.overlays.open_with.is_none(),
            "overlay must remain closed"
        );
        assert_eq!(app.status, "Failed to open with Ghost App");

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn single_terminal_app_queues_pending_command_without_overlay() {
        let root = temp_dir_path("terminal-single-root");
        fs::create_dir_all(&root).expect("create temp root");
        let path = root.join("file.txt");
        fs::write(&path, "hello").expect("write temp file");

        let mut app = App::new_at(root.clone()).expect("create app");
        app.handle_discovered_open_with_apps(
            &path,
            vec![fake_terminal_app("Neovim")],
            |_| unreachable!("fallback should not be called"),
            |_| unreachable!("detached launch should not be called for terminal apps"),
        );

        assert!(
            app.overlays.open_with.is_none(),
            "overlay must remain closed for direct terminal launch"
        );
        assert_eq!(
            app.pending_terminal_command,
            Some(("nvim".to_string(), vec!["/tmp/file.txt".to_string()]))
        );
        assert!(app.status.is_empty());

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn confirm_terminal_app_from_overlay_queues_pending_command() {
        let root = temp_dir_path("terminal-overlay-root");
        fs::create_dir_all(&root).expect("create temp root");
        fs::write(root.join("file.txt"), "hello").expect("write temp file");

        let mut app = App::new_at(root.clone()).expect("create app");
        // Put two apps in the overlay (terminal + gui) so it opens.
        app.handle_discovered_open_with_apps(
            &root.join("file.txt"),
            vec![fake_terminal_app("Neovim"), fake_open_with_app("Gedit")],
            |_| unreachable!(),
            |_| unreachable!(),
        );
        assert!(app.overlays.open_with.is_some(), "overlay should be open");

        // The first row is the terminal app. Confirm it.
        app.confirm_open_with_index(0)
            .expect("confirm should not error");

        assert!(app.overlays.open_with.is_none(), "overlay must close");
        assert_eq!(
            app.pending_terminal_command,
            Some(("nvim".to_string(), vec!["/tmp/file.txt".to_string()]))
        );
        assert!(app.status.is_empty());

        fs::remove_dir_all(root).ok();
    }
}
