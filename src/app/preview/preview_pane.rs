use super::App;
use crate::config;
use ratatui::text::Line;
use std::sync::Arc;

impl App {
    pub(crate) fn preview_visible(&self) -> bool {
        self.preview.visible
    }

    pub(crate) fn preview_fullscreen(&self) -> bool {
        self.preview.visible && self.preview.fullscreen
    }

    pub(crate) fn toggle_fullscreen_preview(&mut self) {
        if preview_pane_disabled_by_layout(config::layout()) {
            self.preview.visible = false;
            self.preview.fullscreen = false;
            self.status = "Preview pane disabled in config".to_string();
            return;
        }

        if !self.preview.visible {
            self.preview.visible = true;
            self.refresh_preview();
        }

        let next_fullscreen = !self.preview.fullscreen;
        if self.preview.fullscreen != next_fullscreen {
            self.queue_terminal_image_geometry_clear();
        }
        self.preview.fullscreen = next_fullscreen;
        self.status = if self.preview.fullscreen {
            "Fullscreen preview"
        } else {
            "Exited fullscreen preview"
        }
        .to_string();
    }

    pub(crate) fn exit_fullscreen_preview(&mut self) -> bool {
        if self.clear_fullscreen_preview() {
            self.status = "Exited fullscreen preview".to_string();
            true
        } else {
            false
        }
    }

    pub(crate) fn clear_fullscreen_preview(&mut self) -> bool {
        if self.preview.fullscreen {
            self.queue_terminal_image_geometry_clear();
            self.preview.fullscreen = false;
            self.preview.exit_fullscreen_after_directory_load = false;
            true
        } else {
            false
        }
    }

    pub(crate) fn toggle_preview_pane(&mut self) {
        if preview_pane_disabled_by_layout(config::layout()) {
            self.preview.visible = false;
            self.preview.fullscreen = false;
            self.status = "Preview pane disabled in config".to_string();
            return;
        }

        self.preview.visible = !self.preview.visible;
        if self.preview.visible {
            self.refresh_preview();
            self.status = "Preview shown".to_string();
        } else {
            self.preview.fullscreen = false;
            self.status = "Preview hidden".to_string();
        }
    }

    pub fn preview_lines(&self) -> Vec<Line<'static>> {
        self.preview.state.content.lines()
    }

    pub fn preview_wrapped_lines(&self, visible_cols: usize) -> Arc<[Line<'static>]> {
        self.preview.state.content.wrapped_lines(visible_cols)
    }

    pub fn preview_section_label(&self) -> &'static str {
        self.preview.state.content.section_label()
    }

    pub fn preview_scroll_offset(&self) -> usize {
        self.preview.state.scroll
    }

    pub fn preview_horizontal_scroll_offset(&self) -> usize {
        self.preview.state.horizontal_scroll
    }

    pub fn preview_total_lines(&self, visible_cols: usize) -> usize {
        self.preview.state.content.visual_line_count(visible_cols)
    }

    pub fn preview_wraps(&self) -> bool {
        self.preview.state.content.kind.wraps_in_preview()
    }

    pub fn preview_allows_horizontal_scroll(&self) -> bool {
        self.preview.state.content.kind.allows_horizontal_scroll()
    }

    pub fn preview_max_horizontal_scroll(&self, visible_cols: usize) -> usize {
        if !self.preview_allows_horizontal_scroll() {
            return 0;
        }
        self.preview
            .state
            .content
            .wrapped_max_line_width(visible_cols)
            .saturating_sub(visible_cols.max(1))
    }

    #[cfg(test)]
    pub fn preview_header_detail(&self, visible_rows: usize) -> Option<String> {
        let visible_cols = self.input.screen_regions.preview_cols_visible;
        let detail = self
            .preview
            .state
            .content
            .header_detail(self.preview.state.scroll, visible_rows);
        let wrapped_note =
            if self.preview.state.content.truncation_note.is_none() && visible_cols > 0 {
                self.preview
                    .state
                    .content
                    .wrapped_truncation_note(visible_cols)
            } else {
                None
            };
        let mut detail = match (detail, wrapped_note) {
            (Some(detail), Some(note)) if !note.is_empty() => Some(format!("{detail}  •  {note}")),
            (Some(detail), Some(_)) => Some(detail),
            (Some(detail), None) => Some(detail),
            (None, Some(note)) => Some(note),
            (None, None) => None,
        };
        if let Some(navigation_detail) = self.preview.state.content.navigation_header_detail() {
            detail = Some(match detail {
                Some(detail) if !detail.is_empty() => format!("{detail}  •  {navigation_detail}"),
                _ => navigation_detail,
            });
        }
        if let Some(pdf_detail) = self.pdf_preview_header_detail() {
            detail = Some(match detail {
                Some(detail) if !detail.is_empty() => format!("{detail}  •  {pdf_detail}"),
                _ => pdf_detail,
            });
        }
        if let Some(image_detail) = self.static_image_preview_header_detail() {
            detail = Some(match detail {
                Some(detail) if !detail.is_empty() => format!("{detail}  •  {image_detail}"),
                _ => image_detail,
            });
        }
        detail
    }

    pub(crate) fn preview_header_detail_for_width(
        &self,
        visible_rows: usize,
        available_width: usize,
    ) -> Option<String> {
        let pdf_detail = self.pdf_preview_header_detail();
        let image_detail = self.static_image_preview_header_detail();
        self.preview.header_detail_for_width(
            visible_rows,
            self.input.screen_regions.preview_cols_visible,
            pdf_detail.as_deref(),
            image_detail.as_deref(),
            available_width,
        )
    }
}

pub(super) fn preview_pane_disabled_by_layout(layout: config::LayoutConfig) -> bool {
    layout.panes.is_some_and(|panes| panes.preview == 0)
}
