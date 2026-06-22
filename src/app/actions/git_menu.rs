use super::super::{
    App,
    git::{GitCommand, GitMenuAction, GitRemote},
    state::{GitMenuOverlay, GitMenuOverlayRow},
};
use crate::fs::rect_contains;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

impl App {
    pub fn git_menu_is_open(&self) -> bool {
        self.overlays.git_menu.is_some()
    }

    pub fn git_menu_title(&self) -> &str {
        self.overlays
            .git_menu
            .as_ref()
            .map(|overlay| overlay.title.as_str())
            .unwrap_or("")
    }

    pub fn git_menu_row_count(&self) -> usize {
        self.overlays
            .git_menu
            .as_ref()
            .map(|overlay| overlay.rows.len())
            .unwrap_or(0)
    }

    pub fn git_menu_row_label(&self, index: usize) -> &str {
        self.overlays
            .git_menu
            .as_ref()
            .and_then(|overlay| overlay.rows.get(index))
            .map(|row| row.label.as_str())
            .unwrap_or("")
    }

    pub fn git_menu_row_shortcut(&self, index: usize) -> Option<char> {
        self.overlays
            .git_menu
            .as_ref()
            .and_then(|overlay| overlay.rows.get(index))
            .map(|row| row.shortcut)
    }
}

impl App {
    pub(in crate::app) fn open_git_menu_overlay(&mut self) {
        if self.git_branch().is_none() {
            self.status = "Not a git repository".to_string();
            return;
        }
        self.overlays.help = false;
        self.overlays.git_menu = Some(build_git_menu_overlay());
        self.status.clear();
    }

    pub(in crate::app) fn handle_git_menu_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.overlays.git_menu = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.overlays.git_menu = None;
            }
            _ => {
                if let Some(index) = crate::config::normalized_plain_key_char(key)
                    .and_then(|ch| self.git_menu_row_index_for_shortcut(ch))
                {
                    self.confirm_git_menu_index(index);
                }
            }
        }

        Ok(())
    }

    pub(in crate::app) fn handle_git_menu_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let inside = self
                .input
                .frame_state
                .git_menu_panel
                .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
            if !inside {
                self.overlays.git_menu = None;
                return Ok(());
            }

            if let Some(hit) = self
                .input
                .frame_state
                .git_menu_hits
                .iter()
                .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                .cloned()
            {
                self.confirm_git_menu_index(hit.index);
            }
        }

        Ok(())
    }

    fn git_menu_row_index_for_shortcut(&self, ch: char) -> Option<usize> {
        self.overlays
            .git_menu
            .as_ref()
            .and_then(|overlay| overlay.rows.iter().position(|row| row.shortcut == ch))
    }

    fn confirm_git_menu_index(&mut self, index: usize) {
        let Some(action) = self
            .overlays
            .git_menu
            .as_ref()
            .and_then(|overlay| overlay.rows.get(index).map(|row| row.action))
        else {
            return;
        };
        self.overlays.git_menu = None;
        match action {
            GitMenuAction::View(command) => self.run_git_command(command),
            GitMenuAction::Stage => self.run_git_stage(false),
            GitMenuAction::Unstage => self.run_git_stage(true),
            GitMenuAction::Commit => self.open_commit_prompt(),
            GitMenuAction::Remote(remote) => self.run_git_remote(remote),
        }
    }
}

fn build_git_menu_overlay() -> GitMenuOverlay {
    let rows = vec![
        git_menu_row('s', "status", GitMenuAction::View(GitCommand::Status)),
        git_menu_row('l', "log", GitMenuAction::View(GitCommand::Log)),
        git_menu_row('d', "diff", GitMenuAction::View(GitCommand::Diff)),
        git_menu_row('a', "stage file", GitMenuAction::Stage),
        git_menu_row('u', "unstage file", GitMenuAction::Unstage),
        git_menu_row('c', "commit", GitMenuAction::Commit),
        git_menu_row('f', "fetch", GitMenuAction::Remote(GitRemote::Fetch)),
        git_menu_row('p', "pull", GitMenuAction::Remote(GitRemote::Pull)),
        git_menu_row('P', "push", GitMenuAction::Remote(GitRemote::Push)),
    ];

    GitMenuOverlay {
        title: "Git".to_string(),
        rows,
    }
}

fn git_menu_row(shortcut: char, label: &str, action: GitMenuAction) -> GitMenuOverlayRow {
    GitMenuOverlayRow {
        shortcut,
        label: label.to_string(),
        action,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf, time::SystemTime};

    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-{name}-{unique}"))
    }

    fn app_in_repo(name: &str) -> (App, PathBuf) {
        let root = temp_dir(name);
        fs::create_dir_all(&root).expect("failed to create temp dir");
        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.set_git_branch_for_test(Some("main"));
        (app, root)
    }

    fn plain_key(ch: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
    }

    #[test]
    fn opens_with_view_and_stage_commands() {
        let (mut app, root) = app_in_repo("git-menu-open");

        app.open_git_menu_overlay();

        assert!(app.git_menu_is_open());
        assert_eq!(app.git_menu_row_count(), 9);
        assert_eq!(app.git_menu_row_shortcut(0), Some('s'));
        assert_eq!(app.git_menu_row_label(0), "status");
        assert_eq!(app.git_menu_row_shortcut(1), Some('l'));
        assert_eq!(app.git_menu_row_shortcut(2), Some('d'));
        assert_eq!(app.git_menu_row_shortcut(3), Some('a'));
        assert_eq!(app.git_menu_row_label(3), "stage file");
        assert_eq!(app.git_menu_row_shortcut(4), Some('u'));
        assert_eq!(app.git_menu_row_label(4), "unstage file");
        assert_eq!(app.git_menu_row_shortcut(5), Some('c'));
        assert_eq!(app.git_menu_row_label(5), "commit");
        assert_eq!(app.git_menu_row_shortcut(6), Some('f'));
        assert_eq!(app.git_menu_row_label(6), "fetch");
        assert_eq!(app.git_menu_row_shortcut(7), Some('p'));
        assert_eq!(app.git_menu_row_label(7), "pull");
        assert_eq!(app.git_menu_row_shortcut(8), Some('P'));
        assert_eq!(app.git_menu_row_label(8), "push");

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn fetch_entry_submits_a_remote_command() {
        let (mut app, root) = app_in_repo("git-menu-fetch");
        app.open_git_menu_overlay();
        let before = app.git.command_token;

        app.handle_git_menu_key(plain_key('f'))
            .expect("fetch shortcut should be handled");

        assert!(!app.git_menu_is_open());
        assert_eq!(app.git.command_token, before.wrapping_add(1));

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn commit_entry_opens_the_commit_prompt() {
        let (mut app, root) = app_in_repo("git-menu-commit");
        app.open_git_menu_overlay();

        app.handle_git_menu_key(plain_key('c'))
            .expect("commit shortcut should be handled");

        assert!(!app.git_menu_is_open());
        assert!(app.commit_is_open());

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn does_not_open_outside_a_repo() {
        let root = temp_dir("git-menu-no-repo");
        fs::create_dir_all(&root).expect("failed to create temp dir");
        let mut app = App::new_at(root.clone()).expect("failed to create app");

        app.open_git_menu_overlay();

        assert!(!app.git_menu_is_open());
        assert_eq!(app.status, "Not a git repository");

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn selecting_a_command_closes_the_menu_and_submits() {
        let (mut app, root) = app_in_repo("git-menu-select");
        app.open_git_menu_overlay();
        let before = app.git.command_token;

        app.handle_git_menu_key(plain_key('s'))
            .expect("status shortcut should be handled");

        assert!(!app.git_menu_is_open());
        assert_eq!(app.git.command_token, before.wrapping_add(1));

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn staging_a_focused_file_submits() {
        let root = temp_dir("git-menu-stage");
        fs::create_dir_all(&root).expect("failed to create temp dir");
        fs::write(root.join("a.txt"), "x").expect("failed to write file");
        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.set_git_branch_for_test(Some("main"));
        app.open_git_menu_overlay();
        let before = app.git.command_token;

        app.handle_git_menu_key(plain_key('a'))
            .expect("stage shortcut should be handled");

        assert!(!app.git_menu_is_open());
        assert_eq!(app.git.command_token, before.wrapping_add(1));

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn staging_with_no_selection_reports_and_does_not_submit() {
        let (mut app, root) = app_in_repo("git-menu-stage-empty");
        app.open_git_menu_overlay();
        let before = app.git.command_token;

        app.handle_git_menu_key(plain_key('a'))
            .expect("stage shortcut should be handled");

        assert!(!app.git_menu_is_open());
        assert_eq!(app.git.command_token, before);
        assert_eq!(app.status, "No file selected");

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn escape_closes_the_menu_without_submitting() {
        let (mut app, root) = app_in_repo("git-menu-escape");
        app.open_git_menu_overlay();
        let before = app.git.command_token;

        app.handle_git_menu_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .expect("escape should be handled");

        assert!(!app.git_menu_is_open());
        assert_eq!(app.git.command_token, before);

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }
}
