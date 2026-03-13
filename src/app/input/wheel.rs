use super::*;

#[derive(Clone, Copy)]
pub(in crate::app) struct WheelTuning {
    queue_limit: isize,
    medium_threshold: u8,
    fast_threshold: u8,
    medium_divisor: isize,
    fast_divisor: isize,
}

pub(in crate::app) const ENTRY_WHEEL_TUNING: WheelTuning = WheelTuning {
    queue_limit: WHEEL_SCROLL_QUEUE_LIMIT,
    medium_threshold: 4,
    fast_threshold: 8,
    medium_divisor: 2,
    fast_divisor: 3,
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
    queue_limit: 16,
    medium_threshold: 4,
    fast_threshold: 8,
    medium_divisor: 1,
    fast_divisor: 1,
};
const HIGH_FREQUENCY_ENTRY_HORIZONTAL_WHEEL_TUNING: WheelTuning = WheelTuning {
    queue_limit: 2,
    medium_threshold: 2,
    fast_threshold: 3,
    medium_divisor: 3,
    fast_divisor: 5,
};

impl App {
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

    pub(in crate::app) fn handle_wheel_event(&mut self, mouse: MouseEvent, delta: isize) {
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
                } else if self.wheel_profile == WheelProfile::HighFrequency {
                    let _ = self.scroll_entry_immediately(delta);
                } else {
                    let tuning = self.entry_wheel_tuning();
                    Self::queue_scroll(&mut self.wheel_scroll.vertical, delta, tuning);
                }
            }
        }
    }

    pub(in crate::app) fn handle_horizontal_wheel_event(
        &mut self,
        mouse: MouseEvent,
        delta: isize,
    ) {
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

    pub(in crate::app) fn queue_search_wheel(&mut self, delta: isize) {
        Self::queue_scroll(&mut self.wheel_scroll.search, delta, SEARCH_WHEEL_TUNING);
    }

    pub(in crate::app) fn queue_scroll(lane: &mut ScrollLane, delta: isize, tuning: WheelTuning) {
        let burst_count = Self::register_scroll_input(lane, delta);

        let divisor = if burst_count >= tuning.fast_threshold {
            tuning.fast_divisor
        } else if burst_count >= tuning.medium_threshold {
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

    fn register_scroll_input(lane: &mut ScrollLane, delta: isize) -> u8 {
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
        lane.burst_count
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

        let mut dirty = false;
        let total_steps =
            self.entry_vertical_steps_per_flush(self.wheel_scroll.vertical.pending.abs() + 1);
        for step_index in 0..total_steps {
            if step_index > 0 {
                if self.wheel_scroll.vertical.pending.signum() != step {
                    break;
                }
                self.wheel_scroll.vertical.pending -= step;
            }

            let previous = self.selected;
            self.move_vertical(step);
            dirty |= previous != self.selected;
        }
        dirty
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

    fn scroll_entry_immediately(&mut self, delta: isize) -> bool {
        let burst_count = Self::register_scroll_input(&mut self.wheel_scroll.vertical, delta);
        self.browser_wheel_post_burst_pending = true;
        self.wheel_scroll.vertical.pending = 0;
        self.wheel_scroll.vertical.remainder = 0;
        let step = self.high_frequency_entry_step_multiplier(burst_count);
        let preview_mode = if burst_count <= 1 {
            PreviewRefreshMode::Immediate
        } else {
            PreviewRefreshMode::Deferred
        };
        let previous = self.selected;
        self.move_vertical_with_preview_mode(delta * step, preview_mode);
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

    pub(in crate::app) fn sync_preview_scroll(&mut self) -> bool {
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

    pub(in crate::app) fn clear_wheel_scroll(&mut self) {
        Self::reset_scroll_lane(&mut self.wheel_scroll.vertical);
        Self::reset_scroll_lane(&mut self.wheel_scroll.horizontal);
        Self::reset_scroll_lane(&mut self.wheel_scroll.preview);
        Self::reset_scroll_lane(&mut self.wheel_scroll.preview_horizontal);
        Self::reset_scroll_lane(&mut self.wheel_scroll.search);
        self.browser_wheel_post_burst_pending = false;
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

    fn entry_vertical_steps_per_flush(&self, pending: isize) -> usize {
        if self.wheel_profile != WheelProfile::HighFrequency {
            return 1;
        }

        match pending.abs() {
            0..=2 => 1,
            3..=5 => 2,
            6..=10 => 3,
            _ => 4,
        }
    }

    fn high_frequency_entry_step_multiplier(&self, burst_count: u8) -> isize {
        let _ = burst_count;
        1
    }

    pub(in crate::app) fn handle_horizontal_navigation_key(&mut self, delta: isize) -> bool {
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
        self.preview_prefers_pdf_surface()
            || (self.preview_uses_image_overlay() && self.pdf_preview_header_detail().is_some())
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
}
