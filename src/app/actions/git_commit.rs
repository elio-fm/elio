use super::super::{
    App,
    state::CommitOverlay,
    text_edit::{
        char_to_byte, next_delete_end, next_word_start, previous_delete_start, previous_word_start,
        remove_char_range,
    },
};
use crate::fs::rect_contains;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

impl App {
    pub fn commit_is_open(&self) -> bool {
        self.overlays.commit.is_some()
    }

    pub fn commit_input(&self) -> &str {
        self.overlays.commit.as_ref().map_or("", |c| &c.input)
    }

    pub fn commit_cursor_col(&self) -> usize {
        self.overlays.commit.as_ref().map_or(0, |c| c.cursor_col)
    }

    pub fn commit_branch(&self) -> Option<&str> {
        self.overlays
            .commit
            .as_ref()
            .and_then(|c| c.branch.as_deref())
    }

    pub fn commit_error(&self) -> Option<&str> {
        self.overlays
            .commit
            .as_ref()
            .and_then(|c| c.error.as_deref())
    }

    pub(in crate::app) fn open_commit_prompt(&mut self) {
        if self.git_branch().is_none() {
            self.status = "Not a git repository".to_string();
            return;
        }
        self.overlays.help = false;
        self.overlays.git_menu = None;
        self.overlays.commit = Some(CommitOverlay {
            branch: self.git_branch().map(str::to_string),
            input: String::new(),
            cursor_col: 0,
            error: None,
        });
        self.status.clear();
    }

    pub(in crate::app) fn handle_commit_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.overlays.commit = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.overlays.commit = None;
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                self.confirm_commit();
            }
            KeyCode::Left
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(c) = &mut self.overlays.commit {
                    c.cursor_col = previous_word_start(&c.input, c.cursor_col);
                }
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(c) = &mut self.overlays.commit {
                    c.cursor_col = next_word_start(&c.input, c.cursor_col);
                }
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                if let Some(c) = &mut self.overlays.commit {
                    c.cursor_col = c.cursor_col.saturating_sub(1);
                }
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                if let Some(c) = &mut self.overlays.commit {
                    let len = c.input.chars().count();
                    if c.cursor_col < len {
                        c.cursor_col += 1;
                    }
                }
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(c) = &mut self.overlays.commit {
                    c.cursor_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(c) = &mut self.overlays.commit {
                    c.cursor_col = c.input.chars().count();
                }
            }
            KeyCode::Backspace | KeyCode::Char('h' | 'w')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(c) = &mut self.overlays.commit
                    && c.cursor_col > 0
                {
                    let start = previous_delete_start(&c.input, c.cursor_col);
                    remove_char_range(&mut c.input, start, c.cursor_col);
                    c.cursor_col = start;
                    c.error = None;
                }
            }
            KeyCode::Delete
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(c) = &mut self.overlays.commit {
                    let end = next_delete_end(&c.input, c.cursor_col);
                    remove_char_range(&mut c.input, c.cursor_col, end);
                    c.error = None;
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(c) = &mut self.overlays.commit {
                    let end = next_delete_end(&c.input, c.cursor_col);
                    remove_char_range(&mut c.input, c.cursor_col, end);
                    c.error = None;
                }
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if let Some(c) = &mut self.overlays.commit
                    && c.cursor_col > 0
                {
                    let start = char_to_byte(&c.input, c.cursor_col - 1);
                    let end = char_to_byte(&c.input, c.cursor_col);
                    c.input.replace_range(start..end, "");
                    c.cursor_col -= 1;
                    c.error = None;
                }
            }
            KeyCode::Delete if key.modifiers == KeyModifiers::NONE => {
                if let Some(c) = &mut self.overlays.commit {
                    let len = c.input.chars().count();
                    if c.cursor_col < len {
                        let start = char_to_byte(&c.input, c.cursor_col);
                        let end = char_to_byte(&c.input, c.cursor_col + 1);
                        c.input.replace_range(start..end, "");
                        c.error = None;
                    }
                }
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(c) = &mut self.overlays.commit {
                    let byte = char_to_byte(&c.input, c.cursor_col);
                    c.input.insert(byte, ch);
                    c.cursor_col += 1;
                    c.error = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(in crate::app) fn handle_commit_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let inside = self
                .input
                .frame_state
                .commit_panel
                .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
            if !inside {
                self.overlays.commit = None;
            }
        }
        Ok(())
    }

    fn confirm_commit(&mut self) {
        let Some(commit) = &self.overlays.commit else {
            return;
        };
        let message = commit.input.trim().to_string();
        if message.is_empty() {
            if let Some(commit) = &mut self.overlays.commit {
                commit.error = Some("Commit message cannot be empty".to_string());
            }
            return;
        }
        self.overlays.commit = None;
        self.run_git_commit(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf, time::SystemTime};

    fn app_in_repo(name: &str) -> (App, PathBuf) {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("elio-{name}-{unique}"));
        fs::create_dir_all(&root).expect("failed to create temp dir");
        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.set_git_branch_for_test(Some("main"));
        (app, root)
    }

    fn plain_key(ch: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
    }

    #[test]
    fn typing_and_confirming_submits_a_commit() {
        let (mut app, root) = app_in_repo("commit-confirm");
        app.open_commit_prompt();
        assert!(app.commit_is_open());
        assert_eq!(app.commit_branch(), Some("main"));
        let before = app.git.command_token;

        for ch in "hello".chars() {
            app.handle_commit_key(plain_key(ch))
                .expect("char should be handled");
        }
        assert_eq!(app.commit_input(), "hello");
        app.handle_commit_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("enter should be handled");

        assert!(!app.commit_is_open());
        assert_eq!(app.git.command_token, before.wrapping_add(1));

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn empty_message_is_rejected_without_submitting() {
        let (mut app, root) = app_in_repo("commit-empty");
        app.open_commit_prompt();
        let before = app.git.command_token;

        app.handle_commit_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("enter should be handled");

        assert!(app.commit_is_open(), "prompt stays open on empty message");
        assert_eq!(app.git.command_token, before);
        assert_eq!(app.commit_error(), Some("Commit message cannot be empty"));

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn escape_closes_the_prompt() {
        let (mut app, root) = app_in_repo("commit-escape");
        app.open_commit_prompt();

        app.handle_commit_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .expect("escape should be handled");

        assert!(!app.commit_is_open());

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }
}
