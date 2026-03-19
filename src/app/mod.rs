mod actions;
mod constants;
mod create;
mod directory_counts;
mod input;
mod jobs;
mod overlays;
use crate::preview;
mod search;
mod selection;
mod state;
mod text_edit;
mod types;

#[cfg(test)]
use self::jobs::SchedulerMetricsSnapshot;
use self::jobs::{PreviewLineCountRequest, PreviewPriority, PreviewRequest, SearchRequest};
use self::{constants::*, state::*};
use anyhow::Result;
#[cfg(test)]
use ratatui::layout::Rect;
use ratatui::text::Line;
use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

pub use self::state::App;
#[cfg(test)]
pub use self::state::PreviewMetricsSnapshot;
pub(crate) use crate::fs::{
    format_item_count, format_size, format_time_ago, rect_contains, sanitize_terminal_text,
};

pub(crate) use self::types::FileClass;
pub use self::types::{
    Entry, EntryHit, EntryKind, FrameState, PathHit, SearchHit, SearchRow, SearchScope,
    SidebarItem, SortMode, ViewMetrics, ViewMode,
};

impl App {
    pub fn set_frame_state(&mut self, frame_state: FrameState) -> bool {
        self.frame_state = frame_state;
        let dirty = self.sync_scroll() | self.sync_search_scroll() | self.sync_preview_scroll();
        if !self.browser_wheel_burst_active() {
            self.queue_visible_directory_item_counts();
        }
        self.refresh_static_image_preloads_if_needed();
        self.remember_current_directory_view();
        dirty
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.entries.get(self.selected)
    }

    pub fn has_pending_auto_reload(&self) -> bool {
        self.directory_runtime.pending_reload_at.is_some()
    }

    pub fn has_pending_background_work(&self) -> bool {
        self.scheduler.has_pending_work()
    }

    pub(crate) fn browser_wheel_burst_active(&self) -> bool {
        self.wheel_profile == WheelProfile::HighFrequency
            && self.search.is_none()
            && self.last_wheel_target == Some(WheelTarget::Entries)
            && self
                .wheel_scroll
                .vertical
                .last_input_at
                .is_some_and(|at| at.elapsed() <= WHEEL_SCROLL_BURST_WINDOW)
    }

    pub(crate) fn pending_browser_wheel_timer(&self) -> Option<Duration> {
        if !self.browser_wheel_post_burst_pending {
            return None;
        }
        self.wheel_scroll
            .vertical
            .last_input_at
            .map(|at| WHEEL_SCROLL_BURST_WINDOW.saturating_sub(at.elapsed()))
    }

    pub(crate) fn process_browser_wheel_timers(&mut self) -> bool {
        if self.browser_wheel_post_burst_pending && !self.browser_wheel_burst_active() {
            self.browser_wheel_post_burst_pending = false;
            return true;
        }
        false
    }

    #[cfg(test)]
    pub fn scheduler_metrics(&self) -> SchedulerMetricsSnapshot {
        self.scheduler.metrics_snapshot()
    }

    #[cfg(test)]
    pub fn preview_metrics(&self) -> PreviewMetricsSnapshot {
        self.preview_state.metrics.snapshot()
    }

    pub fn report_runtime_error(&mut self, context: &str, error: &anyhow::Error) {
        self.status = format!("{context}: {error}");
    }
}
