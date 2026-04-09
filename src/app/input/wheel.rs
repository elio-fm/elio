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

        if self.overlays.search.is_some() {
            self.input.wheel_scroll.vertical.pending = 0;
            self.input.wheel_scroll.horizontal.pending = 0;
            self.input.wheel_scroll.preview.pending = 0;
            self.input.wheel_scroll.preview_horizontal.pending = 0;
            dirty |= self.flush_search_scroll();
        } else {
            self.input.wheel_scroll.search.pending = 0;
            dirty |= self.flush_entry_vertical_scroll();
            dirty |= self.flush_preview_scroll();
            dirty |= self.flush_preview_horizontal_scroll();
            if self.navigation.view_mode == ViewMode::Grid {
                dirty |= self.flush_entry_horizontal_scroll();
            } else {
                self.input.wheel_scroll.horizontal.pending = 0;
            }
        }

        dirty
    }

    pub fn has_pending_scroll(&self) -> bool {
        self.input.wheel_scroll.vertical.pending != 0
            || self.input.wheel_scroll.horizontal.pending != 0
            || self.input.wheel_scroll.preview.pending != 0
            || self.input.wheel_scroll.preview_horizontal.pending != 0
            || self.input.wheel_scroll.search.pending != 0
    }

    pub(in crate::app) fn handle_wheel_event(&mut self, mouse: MouseEvent, delta: isize) {
        // In HighFrequency mode (Alacritty, Ghostty, VTE), scroll event coordinates can be
        // unreliable. hover_panel is tracked exclusively from MouseEventKind::Moved events
        // (via ?1003h any-event tracking), which always carry the true cursor position.
        // Use it as the primary routing source; fall back to scroll event coords, then auto-focus.
        let target = if self.input.wheel_profile == WheelProfile::HighFrequency {
            self.input
                .hover_panel
                .or_else(|| self.resolve_wheel_target(mouse.column, mouse.row))
                .or_else(|| self.preview_auto_focus_target(false))
        } else {
            self.resolve_wheel_target(mouse.column, mouse.row)
                .or_else(|| self.preview_auto_focus_target(false))
        };
        match target {
            Some(WheelTarget::Preview) => {
                self.focus_preview_scroll();
                if self.pdf_page_wheel_navigation_active() && self.step_pdf_page(delta) {
                    return;
                }
                if self.comic_page_wheel_navigation_active()
                    && self.step_comic_page_with_preview_mode(delta, PreviewRefreshMode::Deferred)
                {
                    return;
                }
                if self.step_epub_section_from_preview_wheel(delta) {
                    return;
                }
                if mouse.modifiers.contains(KeyModifiers::SHIFT)
                    && self.preview_allows_horizontal_scroll()
                {
                    if self.input.wheel_profile == WheelProfile::HighFrequency {
                        let _ = self.scroll_preview_columns_immediately(delta);
                    } else {
                        Self::queue_scroll(
                            &mut self.input.wheel_scroll.preview_horizontal,
                            delta,
                            PREVIEW_HORIZONTAL_WHEEL_TUNING,
                        );
                    }
                } else if self.input.wheel_profile == WheelProfile::HighFrequency {
                    let _ = self.scroll_preview_immediately(delta);
                } else {
                    Self::queue_scroll(
                        &mut self.input.wheel_scroll.preview,
                        delta,
                        PREVIEW_WHEEL_TUNING,
                    );
                }
            }
            Some(WheelTarget::Entries) | None => {
                self.focus_entry_scroll();
                if self.navigation.view_mode == ViewMode::Grid
                    && mouse.modifiers.contains(KeyModifiers::SHIFT)
                {
                    let tuning = self.entry_horizontal_wheel_tuning();
                    Self::queue_scroll(&mut self.input.wheel_scroll.horizontal, delta, tuning);
                } else if self.input.wheel_profile == WheelProfile::HighFrequency {
                    let _ = self.scroll_entry_immediately(delta);
                } else {
                    let tuning = self.entry_wheel_tuning();
                    Self::queue_scroll(&mut self.input.wheel_scroll.vertical, delta, tuning);
                }
            }
        }
    }

    pub(in crate::app) fn handle_horizontal_wheel_event(
        &mut self,
        mouse: MouseEvent,
        delta: isize,
    ) {
        let target = if self.input.wheel_profile == WheelProfile::HighFrequency {
            self.input
                .hover_panel
                .or_else(|| self.resolve_wheel_target(mouse.column, mouse.row))
                .or_else(|| self.preview_auto_focus_target(true))
        } else {
            self.resolve_wheel_target(mouse.column, mouse.row)
                .or_else(|| self.preview_auto_focus_target(true))
        };
        match target {
            Some(WheelTarget::Preview) => {
                self.focus_preview_scroll();
                if self.preview_allows_horizontal_scroll() {
                    if self.input.wheel_profile == WheelProfile::HighFrequency {
                        let _ = self.scroll_preview_columns_immediately(delta);
                    } else {
                        Self::queue_scroll(
                            &mut self.input.wheel_scroll.preview_horizontal,
                            delta,
                            PREVIEW_HORIZONTAL_WHEEL_TUNING,
                        );
                    }
                }
            }
            Some(WheelTarget::Entries) | None => {
                if self.navigation.view_mode != ViewMode::Grid {
                    return;
                }
                self.focus_entry_scroll();
                let tuning = self.entry_horizontal_wheel_tuning();
                Self::queue_scroll(&mut self.input.wheel_scroll.horizontal, delta, tuning);
            }
        }
    }

    pub(in crate::app) fn queue_search_wheel(&mut self, delta: isize) {
        Self::queue_scroll(
            &mut self.input.wheel_scroll.search,
            delta,
            SEARCH_WHEEL_TUNING,
        );
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
        self.input.last_wheel_target = Some(WheelTarget::Preview);
        Self::reset_scroll_lane(&mut self.input.wheel_scroll.vertical);
        Self::reset_scroll_lane(&mut self.input.wheel_scroll.horizontal);
    }

    fn focus_entry_scroll(&mut self) {
        self.input.last_wheel_target = Some(WheelTarget::Entries);
        Self::reset_scroll_lane(&mut self.input.wheel_scroll.preview);
        Self::reset_scroll_lane(&mut self.input.wheel_scroll.preview_horizontal);
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
        let Some(step) = Self::consume_scroll_step(&mut self.input.wheel_scroll.vertical, interval)
        else {
            return false;
        };

        let mut dirty = false;
        let total_steps =
            self.entry_vertical_steps_per_flush(self.input.wheel_scroll.vertical.pending.abs() + 1);
        for step_index in 0..total_steps {
            if step_index > 0 {
                if self.input.wheel_scroll.vertical.pending.signum() != step {
                    break;
                }
                self.input.wheel_scroll.vertical.pending -= step;
            }

            let previous = self.navigation.selected;
            self.move_vertical_with_preview_mode(step, self.entry_wheel_preview_mode());
            dirty |= previous != self.navigation.selected;
        }
        dirty
    }

    fn flush_entry_horizontal_scroll(&mut self) -> bool {
        let Some(step) = Self::consume_scroll_step(
            &mut self.input.wheel_scroll.horizontal,
            WHEEL_SCROLL_INTERVAL_HORIZONTAL,
        ) else {
            return false;
        };

        let previous = self.navigation.selected;
        self.move_by(step);
        previous != self.navigation.selected
    }

    fn scroll_entry_immediately(&mut self, delta: isize) -> bool {
        let burst_count = Self::register_scroll_input(&mut self.input.wheel_scroll.vertical, delta);
        self.input.browser_wheel_post_burst_pending = true;
        self.input.wheel_scroll.vertical.pending = 0;
        self.input.wheel_scroll.vertical.remainder = 0;
        let step = self.high_frequency_entry_step_multiplier(burst_count);
        let preview_mode = if burst_count <= 1 {
            self.entry_wheel_preview_mode()
        } else {
            PreviewRefreshMode::Deferred
        };
        let previous = self.navigation.selected;
        self.move_vertical_with_preview_mode(delta * step, preview_mode);
        previous != self.navigation.selected
    }

    fn entry_wheel_preview_mode(&self) -> PreviewRefreshMode {
        if self.needs_slow_sixel_navigation_workaround() {
            PreviewRefreshMode::Deferred
        } else {
            PreviewRefreshMode::Immediate
        }
    }

    fn flush_search_scroll(&mut self) -> bool {
        let Some(step) = Self::consume_scroll_step(
            &mut self.input.wheel_scroll.search,
            WHEEL_SCROLL_INTERVAL_SEARCH,
        ) else {
            return false;
        };

        let previous = self
            .overlays
            .search
            .as_ref()
            .map(|search| search.selected)
            .unwrap_or(0);
        self.move_search_selection(step);
        self.overlays
            .search
            .as_ref()
            .map(|search| search.selected != previous)
            .unwrap_or(false)
    }

    fn flush_preview_scroll(&mut self) -> bool {
        let Some(step) = Self::consume_scroll_step(
            &mut self.input.wheel_scroll.preview,
            WHEEL_SCROLL_INTERVAL_PREVIEW,
        ) else {
            return false;
        };
        let mut dirty = self.scroll_preview_lines(step);
        if self.input.wheel_scroll.preview.pending.signum() == step {
            self.input.wheel_scroll.preview.pending -= step;
            dirty |= self.scroll_preview_lines(step);
        }
        dirty
    }

    fn flush_preview_horizontal_scroll(&mut self) -> bool {
        let Some(step) = Self::consume_scroll_step(
            &mut self.input.wheel_scroll.preview_horizontal,
            WHEEL_SCROLL_INTERVAL_PREVIEW_HORIZONTAL,
        ) else {
            return false;
        };
        let mut dirty = self.scroll_preview_columns(step);
        if self.input.wheel_scroll.preview_horizontal.pending.signum() == step {
            self.input.wheel_scroll.preview_horizontal.pending -= step;
            dirty |= self.scroll_preview_columns(step);
        }
        dirty
    }

    fn preview_scroll_step(&self) -> usize {
        self.input
            .frame_state
            .preview_rows_visible
            .saturating_div(6)
            .clamp(2, 4)
    }

    fn preview_horizontal_scroll_step(&self) -> usize {
        self.input
            .frame_state
            .preview_cols_visible
            .saturating_div(8)
            .clamp(2, 6)
    }

    pub(in crate::app) fn sync_preview_scroll(&mut self) -> bool {
        let previous = self.preview.state.scroll;
        let previous_horizontal = self.preview.state.horizontal_scroll;
        let visible_rows = self.input.frame_state.preview_rows_visible;
        let visible_cols = self.input.frame_state.preview_cols_visible;
        let max_scroll = self
            .preview_total_lines(visible_cols)
            .saturating_sub(visible_rows.max(1));
        self.preview.state.scroll = self.preview.state.scroll.min(max_scroll);
        let max_horizontal = self.preview_max_horizontal_scroll(visible_cols);
        self.preview.state.horizontal_scroll =
            self.preview.state.horizontal_scroll.min(max_horizontal);
        // When scroll is clamped at the rendered boundary, fire an extension.
        self.maybe_request_code_preview_extension();
        previous != self.preview.state.scroll
            || previous_horizontal != self.preview.state.horizontal_scroll
    }

    /// Fire an incremental extension job when the user has scrolled close
    /// enough to the bottom of the currently-rendered partial preview.
    fn maybe_request_code_preview_extension(&mut self) {
        if !self.preview.state.content.is_incrementally_partial() {
            return;
        }
        if self.preview.state.incremental_render_in_flight {
            return;
        }
        let Some(render_limit) = self.preview.state.content.incremental_render_limit else {
            return;
        };
        let scroll = self.preview.state.scroll;
        let visible_rows = self.input.frame_state.preview_rows_visible;
        let bottom_edge = scroll.saturating_add(visible_rows);
        if bottom_edge + INCREMENTAL_RENDER_LOOKAHEAD < render_limit {
            return;
        }
        // Build and submit the extension request.
        let Some(entry) = self.selected_entry().cloned() else {
            return;
        };
        let variant = self.current_preview_request_options();
        let Some(request) = self.build_code_preview_extension_request(
            entry.clone(),
            variant,
            PreviewPriority::High,
        ) else {
            return;
        };
        let entry_path = entry.path.clone();
        if self.jobs.scheduler.submit_preview(request) {
            self.preview.state.incremental_render_in_flight = true;
            self.preview.state.incremental_render_path = Some(entry_path);
        }
    }

    pub(in crate::app) fn clear_wheel_scroll(&mut self) {
        Self::reset_scroll_lane(&mut self.input.wheel_scroll.vertical);
        Self::reset_scroll_lane(&mut self.input.wheel_scroll.horizontal);
        Self::reset_scroll_lane(&mut self.input.wheel_scroll.preview);
        Self::reset_scroll_lane(&mut self.input.wheel_scroll.preview_horizontal);
        Self::reset_scroll_lane(&mut self.input.wheel_scroll.search);
        self.input.browser_wheel_post_burst_pending = false;
    }

    fn entry_wheel_tuning(&self) -> WheelTuning {
        match self.input.wheel_profile {
            WheelProfile::Default => ENTRY_WHEEL_TUNING,
            WheelProfile::HighFrequency => HIGH_FREQUENCY_ENTRY_WHEEL_TUNING,
        }
    }

    fn entry_horizontal_wheel_tuning(&self) -> WheelTuning {
        match self.input.wheel_profile {
            WheelProfile::Default => ENTRY_HORIZONTAL_WHEEL_TUNING,
            WheelProfile::HighFrequency => HIGH_FREQUENCY_ENTRY_HORIZONTAL_WHEEL_TUNING,
        }
    }

    fn entry_scroll_interval(&self) -> Duration {
        match self.input.wheel_profile {
            WheelProfile::Default => WHEEL_SCROLL_INTERVAL_VERTICAL,
            WheelProfile::HighFrequency => WHEEL_SCROLL_INTERVAL_VERTICAL_HIGH_FREQUENCY,
        }
    }

    fn entry_vertical_steps_per_flush(&self, pending: isize) -> usize {
        if self.input.wheel_profile != WheelProfile::HighFrequency {
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
        if self.input.last_wheel_target == Some(WheelTarget::Preview) {
            if self.input.wheel_profile == WheelProfile::HighFrequency {
                let _ = self.scroll_preview_columns(delta);
                return true;
            }
            if self.preview_allows_horizontal_scroll()
                && self.preview_max_horizontal_scroll(
                    self.input.frame_state.preview_cols_visible.max(1),
                ) > 0
            {
                return self.scroll_preview_columns(delta);
            }
            self.input.last_wheel_target = Some(WheelTarget::Entries);
        }

        if self.input.wheel_profile == WheelProfile::HighFrequency
            && self.preview_auto_focus_target(true) == Some(WheelTarget::Preview)
            && self.preview_allows_horizontal_scroll()
        {
            self.input.last_wheel_target = Some(WheelTarget::Preview);
            let _ = self.scroll_preview_columns(delta);
            return true;
        }

        if self.input.wheel_profile == WheelProfile::HighFrequency
            && self.navigation.view_mode == ViewMode::Grid
        {
            self.input.last_wheel_target = Some(WheelTarget::Entries);
            self.focus_entry_scroll();
            let tuning = self.entry_horizontal_wheel_tuning();
            Self::queue_scroll(&mut self.input.wheel_scroll.horizontal, delta, tuning);
            return true;
        }

        false
    }

    fn pdf_page_wheel_navigation_active(&self) -> bool {
        self.preview_prefers_pdf_surface()
            || (self.preview_uses_image_overlay() && self.pdf_preview_header_detail().is_some())
    }

    fn comic_page_wheel_navigation_active(&self) -> bool {
        self.comic_preview_wheel_capture_active()
    }

    fn step_epub_section_from_preview_wheel(&mut self, delta: isize) -> bool {
        if !self.epub_section_wheel_navigation_active(delta) {
            return false;
        }

        Self::reset_scroll_lane(&mut self.input.wheel_scroll.preview);
        Self::reset_scroll_lane(&mut self.input.wheel_scroll.preview_horizontal);
        self.step_epub_section(delta)
    }

    fn epub_section_wheel_navigation_active(&self, delta: isize) -> bool {
        if delta == 0 || !self.epub_preview_wheel_capture_active() {
            return false;
        }

        let visible_cols = self.input.frame_state.preview_cols_visible.max(1);
        let visible_rows = self.input.frame_state.preview_rows_visible.max(1);
        let total_lines = self.preview_total_lines(visible_cols);
        if total_lines <= visible_rows {
            return true;
        }

        let max_scroll = total_lines.saturating_sub(visible_rows);
        if delta.is_negative() {
            self.preview.state.scroll == 0
        } else {
            self.preview.state.scroll >= max_scroll
        }
    }

    fn preview_has_vertical_overflow(&self) -> bool {
        let visible_cols = self.input.frame_state.preview_cols_visible.max(1);
        let visible_rows = self.input.frame_state.preview_rows_visible.max(1);
        self.preview_total_lines(visible_cols) > visible_rows
    }

    fn preview_auto_focus_ready(&self) -> bool {
        self.preview_has_vertical_overflow()
            && self.input.last_selection_change_at.elapsed() >= PREVIEW_AUTO_FOCUS_DELAY
    }

    fn preview_horizontal_auto_focus_ready(&self) -> bool {
        self.preview_allows_horizontal_scroll()
            && self
                .preview_max_horizontal_scroll(self.input.frame_state.preview_cols_visible.max(1))
                > 0
            && self.input.last_selection_change_at.elapsed() >= PREVIEW_AUTO_FOCUS_DELAY
    }

    fn preview_auto_focus_target(&self, horizontal: bool) -> Option<WheelTarget> {
        // Fallback only: routes to preview when cursor is outside both panels and the
        // preview has scrollable content. Does NOT use last_wheel_target stickiness —
        // cursor position (via resolve_wheel_target) is always consulted first, so this
        // only fires when the cursor is genuinely ambiguous (e.g. in sidebar/toolbar).
        if self.input.wheel_profile != WheelProfile::HighFrequency {
            return None;
        }

        let preview_ready = if horizontal {
            self.preview_horizontal_auto_focus_ready()
        } else {
            self.preview_auto_focus_ready()
        };

        preview_ready.then_some(WheelTarget::Preview)
    }

    fn scroll_preview_lines(&mut self, delta: isize) -> bool {
        let previous = self.preview.state.scroll;
        let step = self.preview_scroll_step();
        if delta.is_negative() {
            self.preview.state.scroll = self
                .preview
                .state
                .scroll
                .saturating_sub(step.saturating_mul(delta.unsigned_abs()));
        } else {
            self.preview.state.scroll = self
                .preview
                .state
                .scroll
                .saturating_add(step.saturating_mul(delta as usize));
        }
        // sync_preview_scroll already calls maybe_request_code_preview_extension.
        self.sync_preview_scroll();
        previous != self.preview.state.scroll
    }

    pub(in crate::app) fn scroll_preview_columns(&mut self, delta: isize) -> bool {
        let previous = self.preview.state.horizontal_scroll;
        let step = self.preview_horizontal_scroll_step();
        if delta.is_negative() {
            self.preview.state.horizontal_scroll = self
                .preview
                .state
                .horizontal_scroll
                .saturating_sub(step.saturating_mul(delta.unsigned_abs()));
        } else {
            self.preview.state.horizontal_scroll = self
                .preview
                .state
                .horizontal_scroll
                .saturating_add(step.saturating_mul(delta as usize));
        }
        self.sync_preview_scroll();
        previous != self.preview.state.horizontal_scroll
    }

    // Scroll preview immediately without queuing, mirroring scroll_entry_immediately.
    // Used in HighFrequency mode (Alacritty, Ghostty, VTE/Gnome Terminal) where the
    // terminal sends many raw wheel events. The queue system causes lag and stalls in
    // these terminals because it caps pending steps and applies burst throttle divisors.
    fn scroll_preview_immediately(&mut self, delta: isize) -> bool {
        self.input.wheel_scroll.preview.pending = 0;
        self.input.wheel_scroll.preview.remainder = 0;
        self.scroll_preview_lines(delta)
    }

    fn scroll_preview_columns_immediately(&mut self, delta: isize) -> bool {
        self.input.wheel_scroll.preview_horizontal.pending = 0;
        self.input.wheel_scroll.preview_horizontal.remainder = 0;
        self.scroll_preview_columns(delta)
    }
}
