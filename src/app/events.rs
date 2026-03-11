use super::*;
use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::Path;
use std::time::{Duration, Instant};

impl App {
    pub fn handle_event(&mut self, event: Event) -> Result<()> {
        let result = match event {
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::Resize(_, _) | Event::FocusGained | Event::FocusLost | Event::Paste(_) => Ok(()),
        };

        if let Err(error) = result {
            self.report_runtime_error("Action failed", &error);
        }

        Ok(())
    }

    pub fn process_pending_scroll(&mut self) -> bool {
        let mut dirty = false;

        if self.search.is_some() {
            self.wheel_scroll.vertical.pending = 0;
            self.wheel_scroll.horizontal.pending = 0;
            self.wheel_scroll.preview.pending = 0;
            self.wheel_scroll.preview_horizontal.pending = 0;
            dirty |= self.flush_search_scroll();
        } else {
            self.wheel_scroll.search.pending = 0;
            dirty |= self.flush_entry_vertical_scroll();
            dirty |= self.flush_preview_scroll();
            dirty |= self.flush_preview_horizontal_scroll();
            if self.view_mode == ViewMode::Grid {
                dirty |= self.flush_entry_horizontal_scroll();
            } else {
                self.wheel_scroll.horizontal.pending = 0;
            }
        }

        dirty
    }

    pub fn has_pending_scroll(&self) -> bool {
        self.wheel_scroll.vertical.pending != 0
            || self.wheel_scroll.horizontal.pending != 0
            || self.wheel_scroll.preview.pending != 0
            || self.wheel_scroll.preview_horizontal.pending != 0
            || self.wheel_scroll.search.pending != 0
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.search.is_some() {
            return self.handle_search_key(key);
        }

        if self.help_open {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c'))
            {
                self.help_open = false;
                return Ok(());
            }
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') => self.help_open = false,
                _ => {}
            }
            return Ok(());
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.should_quit = true;
            return Ok(());
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('f') => {
                    if let Err(error) = self.open_fuzzy_finder(SearchScope::Files) {
                        self.status = format!("Search unavailable: {error}");
                    }
                    return Ok(());
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    self.adjust_zoom(1);
                    return Ok(());
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    self.adjust_zoom(-1);
                    return Ok(());
                }
                KeyCode::Char('0') => {
                    self.reset_zoom();
                    return Ok(());
                }
                _ => {}
            }
        }

        if key.modifiers.contains(KeyModifiers::ALT) {
            match key.code {
                KeyCode::Left => return self.go_back(),
                KeyCode::Right => return self.go_forward(),
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('?') => {
                self.clear_wheel_scroll();
                self.help_open = true;
            }
            KeyCode::Tab => self.step_pinned_place(1)?,
            KeyCode::BackTab => self.step_pinned_place(-1)?,
            KeyCode::Up | KeyCode::Char('k') => self.move_vertical(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_vertical(1),
            KeyCode::Left | KeyCode::Char('h') => {
                if self.view_mode == ViewMode::Grid {
                    self.move_by(-1);
                } else {
                    self.go_parent()?;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.view_mode == ViewMode::Grid {
                    self.move_by(1);
                } else if self.selected_entry().is_some_and(Entry::is_dir) {
                    self.open_selected()?;
                } else {
                    self.status = "Press Enter to open files".to_string();
                }
            }
            KeyCode::PageUp => self.page(-1),
            KeyCode::PageDown => self.page(1),
            KeyCode::Home => self.select_index(0),
            KeyCode::End => self.select_last(),
            KeyCode::Char('g') => self.select_index(0),
            KeyCode::Char('G') => self.select_last(),
            KeyCode::Enter => self.open_selected()?,
            KeyCode::Backspace => self.go_parent()?,
            KeyCode::Char('v') => {
                self.clear_wheel_scroll();
                self.view_mode = self.view_mode.toggle();
                self.sync_scroll();
                self.status = format!("Switched to {} view", self.view_mode.label());
            }
            KeyCode::Char('s') => {
                self.sort_mode = self.sort_mode.cycle();
                self.reload()?;
                self.status = format!("Sort: {}", self.sort_mode.label());
            }
            KeyCode::Char('.') => {
                self.show_hidden = !self.show_hidden;
                self.reload()?;
                self.status = if self.show_hidden {
                    "Hidden files shown".to_string()
                } else {
                    "Hidden files hidden".to_string()
                };
            }
            KeyCode::Char('f') => {
                if let Err(error) = self.open_fuzzy_finder(SearchScope::Folders) {
                    self.status = format!("Search unavailable: {error}");
                }
            }
            KeyCode::Char('o') => self.open_in_system()?,
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if self.search.is_some() {
            return self.handle_search_mouse(mouse);
        }

        if self.help_open {
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                self.clear_wheel_scroll();
                self.help_open = false;
            }
            return Ok(());
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(rect) = self.frame_state.back_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    return self.go_back();
                }
                if let Some(rect) = self.frame_state.forward_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    return self.go_forward();
                }
                if let Some(rect) = self.frame_state.parent_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    return self.go_parent();
                }
                if let Some(rect) = self.frame_state.hidden_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    self.show_hidden = !self.show_hidden;
                    self.reload()?;
                    self.status = if self.show_hidden {
                        "Hidden files shown".to_string()
                    } else {
                        "Hidden files hidden".to_string()
                    };
                    return Ok(());
                }
                if let Some(rect) = self.frame_state.view_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    self.clear_wheel_scroll();
                    self.view_mode = self.view_mode.toggle();
                    self.sync_scroll();
                    self.status = format!("Switched to {} view", self.view_mode.label());
                    return Ok(());
                }

                if let Some(target) = self
                    .frame_state
                    .sidebar_hits
                    .iter()
                    .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                    .cloned()
                {
                    return self.set_dir(target.path);
                }

                if let Some(hit) = self
                    .frame_state
                    .entry_hits
                    .iter()
                    .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                    .cloned()
                {
                    let Some(path) = self.entries.get(hit.index).map(|entry| entry.path.clone())
                    else {
                        return Ok(());
                    };
                    self.select_index(hit.index);
                    if self.is_double_click(&path) {
                        self.open_selected()?;
                    }
                    self.last_click = Some(ClickState {
                        path,
                        at: Instant::now(),
                    });
                }
            }
            MouseEventKind::ScrollDown => {
                if self
                    .frame_state
                    .preview_panel
                    .is_some_and(|rect| rect_contains(rect, mouse.column, mouse.row))
                {
                    self.focus_preview_scroll();
                    if mouse.modifiers.contains(KeyModifiers::SHIFT)
                        && self.preview_allows_horizontal_scroll()
                    {
                        Self::queue_scroll(
                            &mut self.wheel_scroll.preview_horizontal,
                            1,
                            self.wheel_step_divisor,
                            WHEEL_SCROLL_QUEUE_LIMIT_PREVIEW_HORIZONTAL,
                        );
                    } else {
                        Self::queue_scroll(
                            &mut self.wheel_scroll.preview,
                            1,
                            self.wheel_step_divisor,
                            WHEEL_SCROLL_QUEUE_LIMIT,
                        );
                    }
                    return Ok(());
                }
                self.focus_entry_scroll();
                if self.view_mode == ViewMode::Grid {
                    if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                        Self::queue_scroll(
                            &mut self.wheel_scroll.horizontal,
                            1,
                            self.wheel_step_divisor,
                            WHEEL_SCROLL_QUEUE_LIMIT_HORIZONTAL,
                        );
                    } else {
                        Self::queue_scroll(
                            &mut self.wheel_scroll.vertical,
                            1,
                            self.wheel_step_divisor,
                            WHEEL_SCROLL_QUEUE_LIMIT,
                        );
                    }
                } else {
                    Self::queue_scroll(
                        &mut self.wheel_scroll.vertical,
                        1,
                        self.wheel_step_divisor,
                        WHEEL_SCROLL_QUEUE_LIMIT,
                    );
                }
            }
            MouseEventKind::ScrollUp => {
                if self
                    .frame_state
                    .preview_panel
                    .is_some_and(|rect| rect_contains(rect, mouse.column, mouse.row))
                {
                    self.focus_preview_scroll();
                    if mouse.modifiers.contains(KeyModifiers::SHIFT)
                        && self.preview_allows_horizontal_scroll()
                    {
                        Self::queue_scroll(
                            &mut self.wheel_scroll.preview_horizontal,
                            -1,
                            self.wheel_step_divisor,
                            WHEEL_SCROLL_QUEUE_LIMIT_PREVIEW_HORIZONTAL,
                        );
                    } else {
                        Self::queue_scroll(
                            &mut self.wheel_scroll.preview,
                            -1,
                            self.wheel_step_divisor,
                            WHEEL_SCROLL_QUEUE_LIMIT,
                        );
                    }
                    return Ok(());
                }
                self.focus_entry_scroll();
                if self.view_mode == ViewMode::Grid {
                    if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                        Self::queue_scroll(
                            &mut self.wheel_scroll.horizontal,
                            -1,
                            self.wheel_step_divisor,
                            WHEEL_SCROLL_QUEUE_LIMIT_HORIZONTAL,
                        );
                    } else {
                        Self::queue_scroll(
                            &mut self.wheel_scroll.vertical,
                            -1,
                            self.wheel_step_divisor,
                            WHEEL_SCROLL_QUEUE_LIMIT,
                        );
                    }
                } else {
                    Self::queue_scroll(
                        &mut self.wheel_scroll.vertical,
                        -1,
                        self.wheel_step_divisor,
                        WHEEL_SCROLL_QUEUE_LIMIT,
                    );
                }
            }
            MouseEventKind::ScrollLeft => {
                if self
                    .frame_state
                    .preview_panel
                    .is_some_and(|rect| rect_contains(rect, mouse.column, mouse.row))
                {
                    self.focus_preview_scroll();
                    if self.preview_allows_horizontal_scroll() {
                        Self::queue_scroll(
                            &mut self.wheel_scroll.preview_horizontal,
                            -1,
                            self.wheel_step_divisor,
                            WHEEL_SCROLL_QUEUE_LIMIT_PREVIEW_HORIZONTAL,
                        );
                    }
                    return Ok(());
                }
                if self.view_mode != ViewMode::Grid {
                    return Ok(());
                }
                self.focus_entry_scroll();
                Self::queue_scroll(
                    &mut self.wheel_scroll.horizontal,
                    -1,
                    self.wheel_step_divisor,
                    WHEEL_SCROLL_QUEUE_LIMIT_HORIZONTAL,
                );
            }
            MouseEventKind::ScrollRight => {
                if self
                    .frame_state
                    .preview_panel
                    .is_some_and(|rect| rect_contains(rect, mouse.column, mouse.row))
                {
                    self.focus_preview_scroll();
                    if self.preview_allows_horizontal_scroll() {
                        Self::queue_scroll(
                            &mut self.wheel_scroll.preview_horizontal,
                            1,
                            self.wheel_step_divisor,
                            WHEEL_SCROLL_QUEUE_LIMIT_PREVIEW_HORIZONTAL,
                        );
                    }
                    return Ok(());
                }
                if self.view_mode != ViewMode::Grid {
                    return Ok(());
                }
                self.focus_entry_scroll();
                Self::queue_scroll(
                    &mut self.wheel_scroll.horizontal,
                    1,
                    self.wheel_step_divisor,
                    WHEEL_SCROLL_QUEUE_LIMIT_HORIZONTAL,
                );
            }
            _ => {}
        }
        Ok(())
    }

    pub fn open_selected(&mut self) -> Result<()> {
        let Some(entry) = self.selected_entry() else {
            return Ok(());
        };
        if entry.is_dir() {
            self.set_dir(entry.path.clone())
        } else {
            self.open_in_system()
        }
    }

    pub(super) fn queue_scroll(
        lane: &mut ScrollLane,
        delta: isize,
        step_divisor: isize,
        queue_limit: isize,
    ) {
        if step_divisor <= 1 {
            lane.pending = (lane.pending + delta).clamp(-queue_limit, queue_limit);
            return;
        }

        lane.remainder += delta;
        while lane.remainder.abs() >= step_divisor {
            let step = lane.remainder.signum();
            lane.pending = (lane.pending + step).clamp(-queue_limit, queue_limit);
            lane.remainder -= step * step_divisor;
        }
    }

    fn consume_scroll_step(lane: &mut ScrollLane, cooldown: Duration) -> Option<isize> {
        let now = Instant::now();
        if lane.pending == 0 {
            return None;
        }
        if lane
            .last_step_at
            .is_some_and(|at| now.duration_since(at) < cooldown)
        {
            return None;
        }

        let step = lane.pending.signum();
        lane.pending -= step;
        lane.last_step_at = Some(now);
        Some(step)
    }

    fn focus_preview_scroll(&mut self) {
        Self::reset_scroll_lane(&mut self.wheel_scroll.vertical);
        Self::reset_scroll_lane(&mut self.wheel_scroll.horizontal);
    }

    fn focus_entry_scroll(&mut self) {
        Self::reset_scroll_lane(&mut self.wheel_scroll.preview);
        Self::reset_scroll_lane(&mut self.wheel_scroll.preview_horizontal);
    }

    fn reset_scroll_lane(lane: &mut ScrollLane) {
        lane.pending = 0;
        lane.remainder = 0;
        lane.last_step_at = None;
    }

    fn flush_entry_vertical_scroll(&mut self) -> bool {
        let Some(step) = Self::consume_scroll_step(
            &mut self.wheel_scroll.vertical,
            WHEEL_SCROLL_INTERVAL_VERTICAL,
        ) else {
            return false;
        };

        let previous = self.selected;
        self.move_vertical(step);
        previous != self.selected
    }

    fn flush_entry_horizontal_scroll(&mut self) -> bool {
        let Some(step) = Self::consume_scroll_step(
            &mut self.wheel_scroll.horizontal,
            WHEEL_SCROLL_INTERVAL_HORIZONTAL,
        ) else {
            return false;
        };

        let previous = self.selected;
        self.move_by(step);
        previous != self.selected
    }

    fn flush_search_scroll(&mut self) -> bool {
        let Some(step) =
            Self::consume_scroll_step(&mut self.wheel_scroll.search, WHEEL_SCROLL_INTERVAL_SEARCH)
        else {
            return false;
        };

        let previous = self
            .search
            .as_ref()
            .map(|search| search.selected)
            .unwrap_or(0);
        self.move_search_selection(step);
        self.search
            .as_ref()
            .map(|search| search.selected != previous)
            .unwrap_or(false)
    }

    fn flush_preview_scroll(&mut self) -> bool {
        let Some(step) = Self::consume_scroll_step(
            &mut self.wheel_scroll.preview,
            WHEEL_SCROLL_INTERVAL_PREVIEW,
        ) else {
            return false;
        };

        let previous = self.preview_scroll;
        let delta = self.preview_scroll_step();
        if step < 0 {
            self.preview_scroll = self.preview_scroll.saturating_sub(delta);
        } else {
            self.preview_scroll = self.preview_scroll.saturating_add(delta);
        }
        self.sync_preview_scroll();
        previous != self.preview_scroll
    }

    fn flush_preview_horizontal_scroll(&mut self) -> bool {
        let Some(step) = Self::consume_scroll_step(
            &mut self.wheel_scroll.preview_horizontal,
            WHEEL_SCROLL_INTERVAL_PREVIEW_HORIZONTAL,
        ) else {
            return false;
        };

        let previous = self.preview_horizontal_scroll;
        let delta = self.preview_horizontal_scroll_step();
        if step < 0 {
            self.preview_horizontal_scroll = self.preview_horizontal_scroll.saturating_sub(delta);
        } else {
            self.preview_horizontal_scroll = self.preview_horizontal_scroll.saturating_add(delta);
        }
        self.sync_preview_scroll();
        previous != self.preview_horizontal_scroll
    }

    fn preview_scroll_step(&self) -> usize {
        self.frame_state
            .preview_rows_visible
            .saturating_div(4)
            .clamp(2, 6)
    }

    fn preview_horizontal_scroll_step(&self) -> usize {
        self.frame_state
            .preview_cols_visible
            .saturating_div(18)
            .clamp(1, 3)
    }

    pub(super) fn sync_preview_scroll(&mut self) -> bool {
        let previous = self.preview_scroll;
        let previous_horizontal = self.preview_horizontal_scroll;
        let visible_rows = self.frame_state.preview_rows_visible;
        let visible_cols = self.frame_state.preview_cols_visible;
        let max_scroll = self
            .preview_total_lines(visible_cols)
            .saturating_sub(visible_rows.max(1));
        self.preview_scroll = self.preview_scroll.min(max_scroll);
        let max_horizontal = self.preview_max_horizontal_scroll(visible_cols);
        self.preview_horizontal_scroll = self.preview_horizontal_scroll.min(max_horizontal);
        previous != self.preview_scroll || previous_horizontal != self.preview_horizontal_scroll
    }

    pub(super) fn clear_wheel_scroll(&mut self) {
        self.wheel_scroll.vertical.pending = 0;
        self.wheel_scroll.vertical.remainder = 0;
        self.wheel_scroll.vertical.last_step_at = None;
        self.wheel_scroll.horizontal.pending = 0;
        self.wheel_scroll.horizontal.remainder = 0;
        self.wheel_scroll.horizontal.last_step_at = None;
        self.wheel_scroll.preview.pending = 0;
        self.wheel_scroll.preview.remainder = 0;
        self.wheel_scroll.preview.last_step_at = None;
        self.wheel_scroll.preview_horizontal.pending = 0;
        self.wheel_scroll.preview_horizontal.remainder = 0;
        self.wheel_scroll.preview_horizontal.last_step_at = None;
        self.wheel_scroll.search.pending = 0;
        self.wheel_scroll.search.remainder = 0;
        self.wheel_scroll.search.last_step_at = None;
    }

    fn is_double_click(&self, path: &Path) -> bool {
        self.last_click
            .as_ref()
            .is_some_and(|click| click.path == path && click.at.elapsed() <= DOUBLE_CLICK_WINDOW)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::{
        env, fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        env::temp_dir().join(format!("elio-events-{label}-{unique}"))
    }

    #[test]
    fn right_arrow_does_not_open_selected_file_in_list_view() {
        let root = temp_path("right-file");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let file_path = root.join("note.txt");
        fs::write(&file_path, "hello").expect("failed to write temp file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.view_mode = ViewMode::List;
        app.select_index(0);

        app.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Right,
            KeyModifiers::NONE,
        )))
        .expect("right arrow should be handled");

        assert_eq!(app.cwd, root);
        assert_eq!(
            app.selected_entry().map(|entry| entry.path.as_path()),
            Some(file_path.as_path())
        );
        assert_eq!(app.status_message(), "Press Enter to open files");

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn right_arrow_enters_selected_directory_in_list_view() {
        let root = temp_path("right-dir");
        let child = root.join("child");
        fs::create_dir_all(&child).expect("failed to create temp dirs");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.view_mode = ViewMode::List;
        app.select_index(0);

        app.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Right,
            KeyModifiers::NONE,
        )))
        .expect("right arrow should be handled");

        assert_eq!(app.cwd, child);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn left_arrow_in_list_view_reselects_previous_directory_in_parent() {
        let root = temp_path("left-parent-selection");
        let alpha = root.join("alpha");
        let child = root.join("child");
        fs::create_dir_all(&alpha).expect("failed to create alpha dir");
        fs::create_dir_all(&child).expect("failed to create child dir");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.view_mode = ViewMode::List;
        app.select_index(1);
        app.open_selected()
            .expect("opening selected directory should succeed");

        app.handle_event(Event::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)))
            .expect("left arrow should be handled");

        assert_eq!(app.cwd, root);
        assert_eq!(
            app.selected_entry().map(|entry| entry.path.as_path()),
            Some(child.as_path())
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn go_back_reselects_previous_directory_in_parent() {
        let root = temp_path("history-back-selection");
        let alpha = root.join("alpha");
        let child = root.join("child");
        fs::create_dir_all(&alpha).expect("failed to create alpha dir");
        fs::create_dir_all(&child).expect("failed to create child dir");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.view_mode = ViewMode::List;
        app.select_index(1);
        app.open_selected()
            .expect("opening selected directory should succeed");

        app.go_back().expect("go back should succeed");

        assert_eq!(app.cwd, root);
        assert_eq!(
            app.selected_entry().map(|entry| entry.path.as_path()),
            Some(child.as_path())
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn go_forward_reselects_previous_directory_in_parent() {
        let root = temp_path("history-forward-selection");
        let child = root.join("child");
        fs::create_dir_all(&child).expect("failed to create child dir");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.view_mode = ViewMode::List;
        app.select_index(0);
        app.open_selected()
            .expect("opening selected directory should succeed");
        app.go_back().expect("go back should succeed");

        app.go_forward().expect("go forward should succeed");

        assert_eq!(app.cwd, child);
        assert!(app.selected_entry().is_none());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn preview_horizontal_scroll_works_in_list_view() {
        let root = temp_path("preview-horizontal-list");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let file_path = root.join("long.rs");
        fs::write(
            &file_path,
            "fn main() { let preview_line = \"this line is intentionally long for horizontal preview scrolling\"; }\n",
        )
        .expect("failed to write temp file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.view_mode = ViewMode::List;
        app.select_index(0);
        app.set_frame_state(FrameState {
            preview_panel: Some(Rect {
                x: 0,
                y: 0,
                width: 20,
                height: 8,
            }),
            preview_rows_visible: 6,
            preview_cols_visible: 12,
            ..FrameState::default()
        });

        app.handle_event(Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollRight,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        }))
        .expect("scroll right should be handled");
        assert!(app.process_pending_scroll());
        assert_eq!(app.preview_horizontal_scroll, 1);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn opening_a_removed_directory_does_not_bubble_an_error() {
        let root = temp_path("removed-directory-open");
        let child = root.join("child");
        fs::create_dir_all(&child).expect("failed to create temp dirs");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        fs::remove_dir_all(&child).expect("failed to remove child dir");

        app.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )))
        .expect("stale directory open should be handled");

        assert_eq!(app.cwd, root);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    #[cfg(unix)]
    fn opening_a_protected_directory_reports_permission_denied() {
        let root = temp_path("protected-directory-open");
        let child = root.join("child");
        fs::create_dir_all(&child).expect("failed to create temp dirs");
        fs::set_permissions(&child, fs::Permissions::from_mode(0o000))
            .expect("failed to lock child dir");

        let mut app = App::new_at(root.clone()).expect("failed to create app");

        app.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )))
        .expect("protected directory open should be handled");

        assert_eq!(app.cwd, root);
        assert!(app.status_message().contains("Permission denied"));

        fs::set_permissions(&child, fs::Permissions::from_mode(0o755))
            .expect("failed to unlock child dir");
        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
