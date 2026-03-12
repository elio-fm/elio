use super::*;
use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
struct WheelTuning {
    queue_limit: isize,
    medium_threshold: u8,
    fast_threshold: u8,
    medium_divisor: isize,
    fast_divisor: isize,
}

const ENTRY_WHEEL_TUNING: WheelTuning = WheelTuning {
    queue_limit: WHEEL_SCROLL_QUEUE_LIMIT,
    medium_threshold: 3,
    fast_threshold: 6,
    medium_divisor: 2,
    fast_divisor: 4,
};
const ENTRY_HORIZONTAL_WHEEL_TUNING: WheelTuning = WheelTuning {
    queue_limit: WHEEL_SCROLL_QUEUE_LIMIT_HORIZONTAL,
    medium_threshold: 2,
    fast_threshold: 4,
    medium_divisor: 2,
    fast_divisor: 3,
};
const PREVIEW_WHEEL_TUNING: WheelTuning = WheelTuning {
    queue_limit: WHEEL_SCROLL_QUEUE_LIMIT,
    medium_threshold: 4,
    fast_threshold: 8,
    medium_divisor: 2,
    fast_divisor: 4,
};
const PREVIEW_HORIZONTAL_WHEEL_TUNING: WheelTuning = WheelTuning {
    queue_limit: WHEEL_SCROLL_QUEUE_LIMIT_PREVIEW_HORIZONTAL,
    medium_threshold: 2,
    fast_threshold: 4,
    medium_divisor: 2,
    fast_divisor: 3,
};
const SEARCH_WHEEL_TUNING: WheelTuning = WheelTuning {
    queue_limit: WHEEL_SCROLL_QUEUE_LIMIT_SEARCH,
    medium_threshold: 2,
    fast_threshold: 4,
    medium_divisor: 2,
    fast_divisor: 3,
};
const HIGH_FREQUENCY_ENTRY_WHEEL_TUNING: WheelTuning = WheelTuning {
    queue_limit: 4,
    medium_threshold: 2,
    fast_threshold: 4,
    medium_divisor: 4,
    fast_divisor: 8,
};
const HIGH_FREQUENCY_ENTRY_HORIZONTAL_WHEEL_TUNING: WheelTuning = WheelTuning {
    queue_limit: 2,
    medium_threshold: 2,
    fast_threshold: 3,
    medium_divisor: 3,
    fast_divisor: 5,
};

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
                    self.open_search_with_status(SearchScope::Files);
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

        if self.wheel_profile == WheelProfile::HighFrequency
            && key.modifiers.contains(KeyModifiers::ALT)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
        {
            match key.code {
                KeyCode::Left => {
                    if self.handle_horizontal_navigation_key(-1) {
                        return Ok(());
                    }
                }
                KeyCode::Right => {
                    if self.handle_horizontal_navigation_key(1) {
                        return Ok(());
                    }
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

        if !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            match key.code {
                KeyCode::Char('[') => {
                    if self.step_pdf_page(-1) {
                        return Ok(());
                    }
                }
                KeyCode::Char(']') => {
                    if self.step_pdf_page(1) {
                        return Ok(());
                    }
                }
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
                self.toggle_view_mode();
            }
            KeyCode::Char('s') => self.cycle_sort_mode()?,
            KeyCode::Char('.') => self.toggle_hidden_files()?,
            KeyCode::Char('f') => self.open_search_with_status(SearchScope::Folders),
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
                self.update_wheel_target_from_position(mouse.column, mouse.row);
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
                    self.toggle_hidden_files()?;
                    return Ok(());
                }
                if let Some(rect) = self.frame_state.view_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    self.toggle_view_mode();
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
                self.handle_wheel_event(mouse, 1);
            }
            MouseEventKind::ScrollUp => {
                self.handle_wheel_event(mouse, -1);
            }
            MouseEventKind::ScrollLeft => {
                self.handle_horizontal_wheel_event(mouse, -1);
            }
            MouseEventKind::ScrollRight => {
                self.handle_horizontal_wheel_event(mouse, 1);
            }
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                self.update_wheel_target_from_position(mouse.column, mouse.row);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_wheel_event(&mut self, mouse: MouseEvent, delta: isize) {
        let target = self
            .high_frequency_preview_target(false)
            .or_else(|| self.resolve_wheel_target(mouse.column, mouse.row));
        match target {
            Some(WheelTarget::Preview) => {
                if self.pdf_page_wheel_navigation_active() && self.step_pdf_page(delta) {
                    return;
                }
                self.focus_preview_scroll();
                if mouse.modifiers.contains(KeyModifiers::SHIFT)
                    && self.preview_allows_horizontal_scroll()
                {
                    Self::queue_scroll(
                        &mut self.wheel_scroll.preview_horizontal,
                        delta,
                        PREVIEW_HORIZONTAL_WHEEL_TUNING,
                    );
                } else {
                    Self::queue_scroll(&mut self.wheel_scroll.preview, delta, PREVIEW_WHEEL_TUNING);
                }
            }
            Some(WheelTarget::Entries) | None => {
                self.focus_entry_scroll();
                if self.view_mode == ViewMode::Grid && mouse.modifiers.contains(KeyModifiers::SHIFT)
                {
                    let tuning = self.entry_horizontal_wheel_tuning();
                    Self::queue_scroll(&mut self.wheel_scroll.horizontal, delta, tuning);
                } else {
                    let tuning = self.entry_wheel_tuning();
                    Self::queue_scroll(&mut self.wheel_scroll.vertical, delta, tuning);
                }
            }
        }
    }

    fn handle_horizontal_wheel_event(&mut self, mouse: MouseEvent, delta: isize) {
        let target = self
            .high_frequency_preview_target(true)
            .or_else(|| self.resolve_wheel_target(mouse.column, mouse.row));
        match target {
            Some(WheelTarget::Preview) => {
                self.focus_preview_scroll();
                if self.preview_allows_horizontal_scroll() {
                    Self::queue_scroll(
                        &mut self.wheel_scroll.preview_horizontal,
                        delta,
                        PREVIEW_HORIZONTAL_WHEEL_TUNING,
                    );
                }
            }
            Some(WheelTarget::Entries) | None => {
                if self.view_mode != ViewMode::Grid {
                    return;
                }
                self.focus_entry_scroll();
                let tuning = self.entry_horizontal_wheel_tuning();
                Self::queue_scroll(&mut self.wheel_scroll.horizontal, delta, tuning);
            }
        }
    }

    pub(super) fn queue_search_wheel(&mut self, delta: isize) {
        Self::queue_scroll(&mut self.wheel_scroll.search, delta, SEARCH_WHEEL_TUNING);
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

    fn queue_scroll(lane: &mut ScrollLane, delta: isize, tuning: WheelTuning) {
        let now = Instant::now();
        let direction = delta.signum();
        let continuing_burst = lane.last_input_direction == direction
            && lane
                .last_input_at
                .is_some_and(|at| now.duration_since(at) <= WHEEL_SCROLL_BURST_WINDOW);

        if continuing_burst {
            lane.burst_count = lane.burst_count.saturating_add(1);
        } else {
            lane.remainder = 0;
            lane.burst_count = 1;
        }
        lane.last_input_at = Some(now);
        lane.last_input_direction = direction;

        let divisor = if lane.burst_count >= tuning.fast_threshold {
            tuning.fast_divisor
        } else if lane.burst_count >= tuning.medium_threshold {
            tuning.medium_divisor
        } else {
            1
        };

        if divisor <= 1 {
            lane.pending = (lane.pending + delta).clamp(-tuning.queue_limit, tuning.queue_limit);
            return;
        }

        lane.remainder += delta;
        while lane.remainder.abs() >= divisor {
            let step = lane.remainder.signum();
            lane.pending = (lane.pending + step).clamp(-tuning.queue_limit, tuning.queue_limit);
            lane.remainder -= step * divisor;
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
        self.last_wheel_target = Some(WheelTarget::Preview);
        Self::reset_scroll_lane(&mut self.wheel_scroll.vertical);
        Self::reset_scroll_lane(&mut self.wheel_scroll.horizontal);
    }

    fn focus_entry_scroll(&mut self) {
        self.last_wheel_target = Some(WheelTarget::Entries);
        Self::reset_scroll_lane(&mut self.wheel_scroll.preview);
        Self::reset_scroll_lane(&mut self.wheel_scroll.preview_horizontal);
    }

    fn reset_scroll_lane(lane: &mut ScrollLane) {
        lane.pending = 0;
        lane.remainder = 0;
        lane.last_step_at = None;
        lane.last_input_at = None;
        lane.last_input_direction = 0;
        lane.burst_count = 0;
    }

    fn flush_entry_vertical_scroll(&mut self) -> bool {
        let interval = self.entry_scroll_interval();
        let Some(step) = Self::consume_scroll_step(&mut self.wheel_scroll.vertical, interval)
        else {
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
        let mut dirty = false;
        for _ in 0..2 {
            let Some(step) = Self::consume_scroll_step(
                &mut self.wheel_scroll.preview,
                WHEEL_SCROLL_INTERVAL_PREVIEW,
            ) else {
                break;
            };
            dirty |= self.scroll_preview_lines(step);
        }
        dirty
    }

    fn flush_preview_horizontal_scroll(&mut self) -> bool {
        let mut dirty = false;
        for _ in 0..2 {
            let Some(step) = Self::consume_scroll_step(
                &mut self.wheel_scroll.preview_horizontal,
                WHEEL_SCROLL_INTERVAL_PREVIEW_HORIZONTAL,
            ) else {
                break;
            };
            dirty |= self.scroll_preview_columns(step);
        }
        dirty
    }

    fn preview_scroll_step(&self) -> usize {
        self.frame_state
            .preview_rows_visible
            .saturating_div(6)
            .clamp(2, 4)
    }

    fn preview_horizontal_scroll_step(&self) -> usize {
        self.frame_state
            .preview_cols_visible
            .saturating_div(20)
            .clamp(1, 3)
    }

    pub(super) fn sync_preview_scroll(&mut self) -> bool {
        let previous = self.preview_state.scroll;
        let previous_horizontal = self.preview_state.horizontal_scroll;
        let visible_rows = self.frame_state.preview_rows_visible;
        let visible_cols = self.frame_state.preview_cols_visible;
        let max_scroll = self
            .preview_total_lines(visible_cols)
            .saturating_sub(visible_rows.max(1));
        self.preview_state.scroll = self.preview_state.scroll.min(max_scroll);
        let max_horizontal = self.preview_max_horizontal_scroll(visible_cols);
        self.preview_state.horizontal_scroll =
            self.preview_state.horizontal_scroll.min(max_horizontal);
        previous != self.preview_state.scroll
            || previous_horizontal != self.preview_state.horizontal_scroll
    }

    pub(super) fn clear_wheel_scroll(&mut self) {
        Self::reset_scroll_lane(&mut self.wheel_scroll.vertical);
        Self::reset_scroll_lane(&mut self.wheel_scroll.horizontal);
        Self::reset_scroll_lane(&mut self.wheel_scroll.preview);
        Self::reset_scroll_lane(&mut self.wheel_scroll.preview_horizontal);
        Self::reset_scroll_lane(&mut self.wheel_scroll.search);
    }

    fn entry_wheel_tuning(&self) -> WheelTuning {
        match self.wheel_profile {
            WheelProfile::Default => ENTRY_WHEEL_TUNING,
            WheelProfile::HighFrequency => HIGH_FREQUENCY_ENTRY_WHEEL_TUNING,
        }
    }

    fn entry_horizontal_wheel_tuning(&self) -> WheelTuning {
        match self.wheel_profile {
            WheelProfile::Default => ENTRY_HORIZONTAL_WHEEL_TUNING,
            WheelProfile::HighFrequency => HIGH_FREQUENCY_ENTRY_HORIZONTAL_WHEEL_TUNING,
        }
    }

    fn entry_scroll_interval(&self) -> Duration {
        match self.wheel_profile {
            WheelProfile::Default => WHEEL_SCROLL_INTERVAL_VERTICAL,
            WheelProfile::HighFrequency => WHEEL_SCROLL_INTERVAL_VERTICAL_HIGH_FREQUENCY,
        }
    }

    fn handle_horizontal_navigation_key(&mut self, delta: isize) -> bool {
        if self.last_wheel_target == Some(WheelTarget::Preview) {
            if self.wheel_profile == WheelProfile::HighFrequency {
                let _ = self.scroll_preview_columns(delta);
                return true;
            }
            if self.preview_allows_horizontal_scroll()
                && self.preview_max_horizontal_scroll(self.frame_state.preview_cols_visible.max(1))
                    > 0
            {
                return self.scroll_preview_columns(delta);
            }
            self.last_wheel_target = Some(WheelTarget::Entries);
        }

        if self.wheel_profile == WheelProfile::HighFrequency
            && self.high_frequency_preview_target(true) == Some(WheelTarget::Preview)
            && self.preview_allows_horizontal_scroll()
        {
            self.last_wheel_target = Some(WheelTarget::Preview);
            let _ = self.scroll_preview_columns(delta);
            return true;
        }

        if self.wheel_profile == WheelProfile::HighFrequency && self.view_mode == ViewMode::Grid {
            self.last_wheel_target = Some(WheelTarget::Entries);
            self.focus_entry_scroll();
            let tuning = self.entry_horizontal_wheel_tuning();
            Self::queue_scroll(&mut self.wheel_scroll.horizontal, delta, tuning);
            return true;
        }

        false
    }

    fn pdf_page_wheel_navigation_active(&self) -> bool {
        self.preview_uses_image_overlay() || self.preview_prefers_pdf_surface()
    }

    fn preview_has_vertical_overflow(&self) -> bool {
        let visible_cols = self.frame_state.preview_cols_visible.max(1);
        let visible_rows = self.frame_state.preview_rows_visible.max(1);
        self.preview_total_lines(visible_cols) > visible_rows
    }

    fn preview_auto_focus_ready(&self) -> bool {
        self.preview_has_vertical_overflow()
            && self.last_selection_change_at.elapsed() >= PREVIEW_AUTO_FOCUS_DELAY
    }

    fn preview_horizontal_auto_focus_ready(&self) -> bool {
        self.preview_allows_horizontal_scroll()
            && self.preview_max_horizontal_scroll(self.frame_state.preview_cols_visible.max(1)) > 0
            && self.last_selection_change_at.elapsed() >= PREVIEW_AUTO_FOCUS_DELAY
    }

    fn high_frequency_preview_target(&self, horizontal: bool) -> Option<WheelTarget> {
        if self.wheel_profile != WheelProfile::HighFrequency {
            return None;
        }

        if self.last_wheel_target == Some(WheelTarget::Preview) {
            return Some(WheelTarget::Preview);
        }

        let preview_ready = if horizontal {
            self.preview_horizontal_auto_focus_ready()
        } else {
            self.preview_auto_focus_ready()
        };

        preview_ready.then_some(WheelTarget::Preview)
    }

    fn scroll_preview_lines(&mut self, delta: isize) -> bool {
        let previous = self.preview_state.scroll;
        let step = self.preview_scroll_step();
        if delta.is_negative() {
            self.preview_state.scroll = self
                .preview_state
                .scroll
                .saturating_sub(step.saturating_mul(delta.unsigned_abs()));
        } else {
            self.preview_state.scroll = self
                .preview_state
                .scroll
                .saturating_add(step.saturating_mul(delta as usize));
        }
        self.sync_preview_scroll();
        previous != self.preview_state.scroll
    }

    fn scroll_preview_columns(&mut self, delta: isize) -> bool {
        let previous = self.preview_state.horizontal_scroll;
        let step = self.preview_horizontal_scroll_step();
        if delta.is_negative() {
            self.preview_state.horizontal_scroll = self
                .preview_state
                .horizontal_scroll
                .saturating_sub(step.saturating_mul(delta.unsigned_abs()));
        } else {
            self.preview_state.horizontal_scroll = self
                .preview_state
                .horizontal_scroll
                .saturating_add(step.saturating_mul(delta as usize));
        }
        self.sync_preview_scroll();
        previous != self.preview_state.horizontal_scroll
    }

    fn panel_target_at(&self, column: u16, row: u16) -> Option<WheelTarget> {
        if self
            .frame_state
            .preview_panel
            .is_some_and(|rect| rect_contains(rect, column, row))
        {
            Some(WheelTarget::Preview)
        } else if self
            .frame_state
            .entries_panel
            .is_some_and(|rect| rect_contains(rect, column, row))
        {
            Some(WheelTarget::Entries)
        } else {
            None
        }
    }

    fn update_wheel_target_from_position(&mut self, column: u16, row: u16) {
        if let Some(target) = self.panel_target_at(column, row) {
            self.last_wheel_target = Some(target);
        }
    }

    fn resolve_wheel_target(&mut self, column: u16, row: u16) -> Option<WheelTarget> {
        if let Some(target) = self.panel_target_at(column, row) {
            self.last_wheel_target = Some(target);
            return Some(target);
        }

        if let Some(preview) = self.frame_state.preview_panel
            && column >= preview.x
        {
            self.last_wheel_target = Some(WheelTarget::Preview);
            return self.last_wheel_target;
        }

        if let Some(entries) = self.frame_state.entries_panel
            && column >= entries.x
            && column < entries.x.saturating_add(entries.width)
        {
            self.last_wheel_target = Some(WheelTarget::Entries);
            return self.last_wheel_target;
        }

        self.last_wheel_target
    }

    fn is_double_click(&self, path: &Path) -> bool {
        self.last_click
            .as_ref()
            .is_some_and(|click| click.path == path && click.at.elapsed() <= DOUBLE_CLICK_WINDOW)
    }
}

#[cfg(test)]
mod tests;
