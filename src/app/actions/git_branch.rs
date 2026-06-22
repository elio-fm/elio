use super::super::{
    App,
    state::BranchPickerOverlay,
    text_edit::{char_to_byte, previous_delete_start, remove_char_range},
};
use crate::fs::rect_contains;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

impl App {
    pub fn branch_picker_is_open(&self) -> bool {
        self.overlays.branch_picker.is_some()
    }

    pub fn branch_query(&self) -> &str {
        self.overlays
            .branch_picker
            .as_ref()
            .map_or("", |b| &b.query)
    }

    pub fn branch_query_cursor(&self) -> usize {
        self.overlays
            .branch_picker
            .as_ref()
            .map_or(0, |b| b.query_cursor)
    }

    pub fn branch_row_count(&self) -> usize {
        self.overlays
            .branch_picker
            .as_ref()
            .map_or(0, |b| b.matches.len())
    }

    pub fn branch_selected_index(&self) -> usize {
        self.overlays
            .branch_picker
            .as_ref()
            .map_or(0, |b| b.selected)
    }

    pub fn branch_row_label(&self, index: usize) -> &str {
        self.overlays
            .branch_picker
            .as_ref()
            .and_then(|b| b.matches.get(index).and_then(|&i| b.branches.get(i)))
            .map_or("", String::as_str)
    }

    pub fn branch_row_is_current(&self, index: usize) -> bool {
        self.overlays.branch_picker.as_ref().is_some_and(|b| {
            b.matches
                .get(index)
                .and_then(|&i| b.branches.get(i))
                .is_some_and(|name| Some(name.as_str()) == b.current.as_deref())
        })
    }

    pub(in crate::app) fn open_branch_picker(&mut self) {
        if self.git_branch().is_none() {
            self.status = "Not a git repository".to_string();
            return;
        }
        if self.git.branches.is_empty() {
            self.status = "No branches found".to_string();
            return;
        }
        let branches = self.git.branches.clone();
        let current = self.git.branch.clone();
        let matches: Vec<usize> = (0..branches.len()).collect();
        // Start on the current branch when present, so Enter on no input is a
        // harmless no-op rather than an accidental switch.
        let selected = current
            .as_ref()
            .and_then(|name| branches.iter().position(|b| b == name))
            .unwrap_or(0);
        self.overlays.help = false;
        self.overlays.git_menu = None;
        self.overlays.branch_picker = Some(BranchPickerOverlay {
            query: String::new(),
            query_cursor: 0,
            branches,
            current,
            matches,
            selected,
        });
        self.status.clear();
    }

    pub(in crate::app) fn handle_branch_picker_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.overlays.branch_picker = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => self.overlays.branch_picker = None,
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => self.confirm_branch_picker(),
            KeyCode::Up if key.modifiers == KeyModifiers::NONE => self.move_branch_selection(-1),
            KeyCode::Down if key.modifiers == KeyModifiers::NONE => self.move_branch_selection(1),
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                if let Some(b) = &mut self.overlays.branch_picker {
                    b.query_cursor = b.query_cursor.saturating_sub(1);
                }
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                if let Some(b) = &mut self.overlays.branch_picker {
                    let len = b.query.chars().count();
                    if b.query_cursor < len {
                        b.query_cursor += 1;
                    }
                }
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(b) = &mut self.overlays.branch_picker {
                    b.query_cursor = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(b) = &mut self.overlays.branch_picker {
                    b.query_cursor = b.query.chars().count();
                }
            }
            KeyCode::Backspace | KeyCode::Char('h' | 'w')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(b) = &mut self.overlays.branch_picker
                    && b.query_cursor > 0
                {
                    let start = previous_delete_start(&b.query, b.query_cursor);
                    remove_char_range(&mut b.query, start, b.query_cursor);
                    b.query_cursor = start;
                }
                self.recompute_branch_matches();
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if let Some(b) = &mut self.overlays.branch_picker
                    && b.query_cursor > 0
                {
                    let start = char_to_byte(&b.query, b.query_cursor - 1);
                    let end = char_to_byte(&b.query, b.query_cursor);
                    b.query.replace_range(start..end, "");
                    b.query_cursor -= 1;
                }
                self.recompute_branch_matches();
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(b) = &mut self.overlays.branch_picker {
                    let byte = char_to_byte(&b.query, b.query_cursor);
                    b.query.insert(byte, ch);
                    b.query_cursor += 1;
                }
                self.recompute_branch_matches();
            }
            _ => {}
        }
        Ok(())
    }

    pub(in crate::app) fn handle_branch_picker_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let inside = self
                .input
                .frame_state
                .branch_panel
                .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
            if !inside {
                self.overlays.branch_picker = None;
                return Ok(());
            }
            if let Some(hit) = self
                .input
                .frame_state
                .branch_hits
                .iter()
                .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                .cloned()
            {
                if let Some(b) = &mut self.overlays.branch_picker {
                    b.selected = hit.index.min(b.matches.len().saturating_sub(1));
                }
                self.confirm_branch_picker();
            }
        }
        Ok(())
    }

    fn move_branch_selection(&mut self, delta: isize) {
        if let Some(b) = &mut self.overlays.branch_picker {
            if b.matches.is_empty() {
                return;
            }
            let max = b.matches.len() as isize - 1;
            let next = (b.selected as isize + delta).clamp(0, max);
            b.selected = next as usize;
        }
    }

    /// Recomputes the filtered match list after a query edit, keeping the
    /// selection on the same branch when it still matches.
    fn recompute_branch_matches(&mut self) {
        let Some(b) = &mut self.overlays.branch_picker else {
            return;
        };
        let selected_name = b
            .matches
            .get(b.selected)
            .and_then(|&i| b.branches.get(i))
            .cloned();
        let query = b.query.to_ascii_lowercase();
        b.matches = b
            .branches
            .iter()
            .enumerate()
            .filter(|(_, name)| name.to_ascii_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        b.selected = selected_name
            .and_then(|name| {
                b.matches
                    .iter()
                    .position(|&i| b.branches.get(i) == Some(&name))
            })
            .unwrap_or(0);
    }

    fn confirm_branch_picker(&mut self) {
        let Some(b) = &self.overlays.branch_picker else {
            return;
        };
        let Some(branch) = b.matches.get(b.selected).and_then(|&i| b.branches.get(i)) else {
            return;
        };
        let branch = branch.clone();
        let already_current = Some(branch.as_str()) == b.current.as_deref();
        self.overlays.branch_picker = None;
        if already_current {
            self.status = format!("Already on {branch}");
            return;
        }
        self.run_git_checkout(branch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf, time::SystemTime};

    fn app_with_branches(name: &str, branches: &[&str], current: &str) -> (App, PathBuf) {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("elio-{name}-{unique}"));
        fs::create_dir_all(&root).expect("failed to create temp dir");
        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.set_git_branch_for_test(Some(current));
        app.set_git_branches_for_test(branches);
        (app, root)
    }

    fn plain_key(ch: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
    }

    #[test]
    fn opens_with_current_branch_selected() {
        let (mut app, root) = app_with_branches("branch-open", &["main", "dev", "feature"], "dev");

        app.open_branch_picker();

        assert!(app.branch_picker_is_open());
        assert_eq!(app.branch_row_count(), 3);
        assert_eq!(app.branch_row_label(app.branch_selected_index()), "dev");
        assert!(app.branch_row_is_current(app.branch_selected_index()));

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn typing_filters_the_list() {
        let (mut app, root) =
            app_with_branches("branch-filter", &["main", "dev", "feature"], "main");
        app.open_branch_picker();

        app.handle_branch_picker_key(plain_key('f'))
            .expect("filter char should be handled");

        assert_eq!(app.branch_row_count(), 1);
        assert_eq!(app.branch_row_label(0), "feature");

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn selecting_a_different_branch_checks_it_out() {
        let (mut app, root) = app_with_branches("branch-switch", &["main", "dev"], "main");
        app.open_branch_picker();
        let before = app.git.command_token;

        // Move from current (main) to dev and confirm.
        app.handle_branch_picker_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .expect("down should be handled");
        app.handle_branch_picker_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("enter should be handled");

        assert!(!app.branch_picker_is_open());
        assert_eq!(app.git.command_token, before.wrapping_add(1));

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }

    #[test]
    fn confirming_current_branch_is_a_noop() {
        let (mut app, root) = app_with_branches("branch-noop", &["main", "dev"], "main");
        app.open_branch_picker();
        let before = app.git.command_token;

        // Selection starts on the current branch (main).
        app.handle_branch_picker_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("enter should be handled");

        assert!(!app.branch_picker_is_open());
        assert_eq!(
            app.git.command_token, before,
            "no checkout should be submitted"
        );
        assert_eq!(app.status, "Already on main");

        fs::remove_dir_all(root).expect("failed to remove temp dir");
    }
}
