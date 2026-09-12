use std::path::PathBuf;

use ratatui::layout::Rect;

use super::App;

pub use crate::fuzzy_finder::SearchScope;

#[derive(Clone, Debug, Default)]
pub struct ScreenRegions {
    pub sidebar_hits: Vec<PathHit>,
    pub entry_hits: Vec<EntryHit>,
    pub search_hits: Vec<SearchHit>,
    pub duplicate_hits: Vec<DuplicateHit>,
    pub goto_hits: Vec<GoToHit>,
    pub copy_hits: Vec<CopyHit>,
    pub open_with_hits: Vec<OpenWithHit>,
    pub trash_panel: Option<Rect>,
    pub trash_confirm_btn: Option<Rect>,
    pub trash_cancel_btn: Option<Rect>,
    pub editor_rename_confirm_btn: Option<Rect>,
    pub editor_rename_cancel_btn: Option<Rect>,
    pub restore_panel: Option<Rect>,
    pub restore_confirm_btn: Option<Rect>,
    pub restore_cancel_btn: Option<Rect>,
    pub archive_create_panel: Option<Rect>,
    pub archive_create_list_area: Option<Rect>,
    pub archive_password_panel: Option<Rect>,
    pub archive_password_visibility_btn: Option<Rect>,
    pub create_panel: Option<Rect>,
    pub rename_panel: Option<Rect>,
    pub create_list_area: Option<Rect>,
    pub create_scroll_top: usize,
    pub bulk_rename_list_area: Option<Rect>,
    pub bulk_rename_scroll_top: usize,
    pub goto_panel: Option<Rect>,
    pub copy_panel: Option<Rect>,
    pub open_with_panel: Option<Rect>,
    pub search_panel: Option<Rect>,
    pub duplicate_panel: Option<Rect>,
    pub help_panel: Option<Rect>,
    pub help_scroll_max: usize,
    pub help_rows_visible: usize,
    pub entries_panel: Option<Rect>,
    pub preview_panel: Option<Rect>,
    pub preview_body_area: Option<Rect>,
    pub preview_media_area: Option<Rect>,
    pub preview_content_area: Option<Rect>,
    pub back_button: Option<Rect>,
    pub forward_button: Option<Rect>,
    pub parent_button: Option<Rect>,
    pub hidden_button: Option<Rect>,
    pub view_button: Option<Rect>,
    pub metrics: ViewMetrics,
    pub preview_rows_visible: usize,
    pub preview_cols_visible: usize,
    pub search_rows_visible: usize,
    pub duplicate_rows_visible: usize,
}

#[derive(Clone, Debug)]
pub struct PathHit {
    pub rect: Rect,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct EntryHit {
    pub rect: Rect,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub rect: Rect,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct DuplicateHit {
    pub rect: Rect,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct GoToHit {
    pub rect: Rect,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct CopyHit {
    pub rect: Rect,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct OpenWithHit {
    pub rect: Rect,
    pub index: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct ViewMetrics {
    pub cols: usize,
    pub rows_visible: usize,
}

impl Default for ViewMetrics {
    fn default() -> Self {
        Self {
            cols: 1,
            rows_visible: 1,
        }
    }
}

impl App {
    pub fn set_screen_regions(&mut self, mut screen_regions: ScreenRegions) -> bool {
        let duplicate_preview_was_rendered = self.duplicate_finder.session.is_some()
            && self.input.screen_regions.preview_panel.is_some();
        let previous_code_line_limit = self.active_preview_entry().map(|entry| {
            self.preview_code_line_limit_for_entry_with_rows(
                &entry,
                self.input.screen_regions.preview_rows_visible,
            )
        });
        if self.preview_fullscreen() && screen_regions.entries_panel.is_none() {
            screen_regions.metrics = self.input.screen_regions.metrics;
        }
        self.input.screen_regions = screen_regions;
        let mut dirty = self.sync_scroll() | self.sync_search_scroll() | self.sync_preview_scroll();
        let duplicate_preview_is_rendered = self.duplicate_finder.session.is_some()
            && self.input.screen_regions.preview_panel.is_some();
        if duplicate_preview_was_rendered && !duplicate_preview_is_rendered {
            self.queue_terminal_image_geometry_clear();
            self.clear_image_preview_selection_activation();
            dirty = true;
        }
        dirty |= self.sync_duplicate_scroll();
        let next_code_line_limit = self.active_preview_entry().map(|entry| {
            self.preview_code_line_limit_for_entry_with_rows(
                &entry,
                self.input.screen_regions.preview_rows_visible,
            )
        });
        if previous_code_line_limit != next_code_line_limit {
            self.refresh_preview();
            dirty = true;
        }
        self.queue_visible_directory_item_counts();
        self.refresh_static_image_preloads_if_needed();
        self.remember_current_directory_view();
        dirty
    }
}
