use super::*;
use std::sync::Arc;

impl App {
    pub fn preview_lines(&self) -> Vec<Line<'static>> {
        self.preview_state.content.lines()
    }

    pub fn preview_wrapped_lines(&self, visible_cols: usize) -> Arc<[Line<'static>]> {
        self.preview_state.content.wrapped_lines(visible_cols)
    }

    pub fn preview_section_label(&self) -> &'static str {
        self.preview_state.content.section_label()
    }

    pub fn preview_scroll_offset(&self) -> usize {
        self.preview_state.scroll
    }

    pub fn preview_horizontal_scroll_offset(&self) -> usize {
        self.preview_state.horizontal_scroll
    }

    pub fn preview_total_lines(&self, visible_cols: usize) -> usize {
        self.preview_state.content.visual_line_count(visible_cols)
    }

    pub fn preview_wraps(&self) -> bool {
        self.preview_state.content.kind.wraps_in_preview()
    }

    pub fn preview_allows_horizontal_scroll(&self) -> bool {
        self.preview_state.content.kind.allows_horizontal_scroll()
    }

    pub fn preview_max_horizontal_scroll(&self, visible_cols: usize) -> usize {
        if !self.preview_allows_horizontal_scroll() {
            return 0;
        }
        self.preview_state
            .content
            .max_line_width()
            .saturating_sub(visible_cols.max(1))
    }

    #[cfg(test)]
    pub fn preview_header_detail(&self, visible_rows: usize) -> Option<String> {
        let visible_cols = self.frame_state.preview_cols_visible;
        let detail = self
            .preview_state
            .content
            .header_detail(self.preview_state.scroll, visible_rows);
        let wrapped_note =
            if self.preview_state.content.truncation_note.is_none() && visible_cols > 0 {
                self.preview_state
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
        if let Some(navigation_detail) = self.preview_state.content.navigation_header_detail() {
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
        let segments = self.preview_header_segments(visible_rows);
        super::headers::fit_preview_header_segments(&segments, available_width)
    }
}
