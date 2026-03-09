use super::*;
use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::Path;
use std::time::{Duration, Instant};

impl App {
    pub fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::Resize(_, _) | Event::FocusGained | Event::FocusLost | Event::Paste(_) => Ok(()),
        }
    }

    pub fn process_pending_scroll(&mut self) -> bool {
        let mut dirty = false;

        if self.search.is_some() {
            self.wheel_scroll.vertical.pending = 0;
            self.wheel_scroll.horizontal.pending = 0;
            dirty |= self.flush_search_scroll();
        } else {
            self.wheel_scroll.search.pending = 0;
            dirty |= self.flush_entry_vertical_scroll();
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
                KeyCode::Char('r') => {
                    self.reload()?;
                    self.status = "Refreshed".to_string();
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
            KeyCode::Up | KeyCode::Char('k') => self.move_vertical(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_vertical(1),
            KeyCode::Left => {
                if self.view_mode == ViewMode::Grid {
                    self.move_by(-1);
                } else {
                    self.go_parent()?;
                }
            }
            KeyCode::Char('h') => self.go_home()?,
            KeyCode::Right | KeyCode::Char('l') => {
                if self.view_mode == ViewMode::Grid {
                    self.move_by(1);
                } else {
                    self.open_selected()?;
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
            KeyCode::Char('r') => {
                self.reload()?;
                self.status = "Refreshed".to_string();
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
                if let Some(rect) = self.frame_state.refresh_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    self.reload()?;
                    self.status = "Refreshed".to_string();
                    return Ok(());
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
                    self.select_index(hit.index);
                    let path = self.entries[hit.index].path.clone();
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
                if self.view_mode == ViewMode::Grid {
                    if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                        Self::queue_scroll(&mut self.wheel_scroll.horizontal, 1);
                    } else {
                        Self::queue_scroll(&mut self.wheel_scroll.vertical, 1);
                    }
                } else {
                    Self::queue_scroll(&mut self.wheel_scroll.vertical, 1);
                }
            }
            MouseEventKind::ScrollUp => {
                if self.view_mode == ViewMode::Grid {
                    if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                        Self::queue_scroll(&mut self.wheel_scroll.horizontal, -1);
                    } else {
                        Self::queue_scroll(&mut self.wheel_scroll.vertical, -1);
                    }
                } else {
                    Self::queue_scroll(&mut self.wheel_scroll.vertical, -1);
                }
            }
            MouseEventKind::ScrollLeft if self.view_mode == ViewMode::Grid => {
                Self::queue_scroll(&mut self.wheel_scroll.horizontal, -1);
            }
            MouseEventKind::ScrollRight if self.view_mode == ViewMode::Grid => {
                Self::queue_scroll(&mut self.wheel_scroll.horizontal, 1);
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

    pub(super) fn queue_scroll(lane: &mut ScrollLane, delta: isize) {
        lane.pending = (lane.pending + delta).clamp(-WHEEL_SCROLL_QUEUE_LIMIT, WHEEL_SCROLL_QUEUE_LIMIT);
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
        let Some(step) = Self::consume_scroll_step(
            &mut self.wheel_scroll.search,
            WHEEL_SCROLL_INTERVAL_SEARCH,
        ) else {
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

    pub(super) fn clear_wheel_scroll(&mut self) {
        self.wheel_scroll.vertical.pending = 0;
        self.wheel_scroll.vertical.last_step_at = None;
        self.wheel_scroll.horizontal.pending = 0;
        self.wheel_scroll.horizontal.last_step_at = None;
        self.wheel_scroll.search.pending = 0;
        self.wheel_scroll.search.last_step_at = None;
    }

    fn is_double_click(&self, path: &Path) -> bool {
        self.last_click
            .as_ref()
            .is_some_and(|click| click.path == path && click.at.elapsed() <= DOUBLE_CLICK_WINDOW)
    }
}
