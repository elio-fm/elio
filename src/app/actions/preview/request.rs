use super::*;
use crate::preview::{
    MIN_DYNAMIC_CODE_PREVIEW_LINE_LIMIT, PreviewRequestOptions, default_code_preview_line_limit,
};

impl App {
    pub(in crate::app) fn build_preview_request(
        &mut self,
        entry: Entry,
        variant: PreviewRequestOptions,
        priority: PreviewPriority,
        work_class: crate::preview::PreviewWorkClass,
    ) -> PreviewRequest {
        let code_line_limit = self.preview_code_line_limit_for_entry(&entry);
        PreviewRequest {
            token: self.preview_state.token,
            entry,
            variant,
            code_line_limit,
            priority,
            work_class,
            ffprobe_available: self.ffprobe_available(),
            ffmpeg_available: self.terminal_image_overlay_available()
                && self.video_ffmpeg_available(),
        }
    }

    pub(in crate::app) fn current_preview_request_options(&self) -> PreviewRequestOptions {
        self.comic_preview_request_options()
            .or_else(|| self.epub_preview_request_options())
            .unwrap_or_default()
    }

    pub(in crate::app) fn preview_code_line_limit_for_entry(&self, entry: &Entry) -> usize {
        self.preview_code_line_limit_for_entry_with_rows(
            entry,
            self.frame_state.preview_rows_visible,
        )
    }

    pub(in crate::app) fn preview_code_line_limit_for_entry_with_rows(
        &self,
        entry: &Entry,
        preview_rows_visible: usize,
    ) -> usize {
        let facts = crate::file_info::inspect_path_cached(
            &entry.path,
            entry.kind,
            entry.size,
            entry.modified,
        );
        if facts.preview.kind == crate::file_info::PreviewKind::Source
            && facts.preview.structured_format.is_none()
        {
            return preview_code_line_limit(preview_rows_visible);
        }
        default_code_preview_line_limit()
    }

    pub(super) fn preview_request_options_for_entry(&self, entry: &Entry) -> PreviewRequestOptions {
        self.comic_preview_request_options_for_entry(entry)
            .or_else(|| self.epub_preview_request_options_for_entry(entry))
            .unwrap_or_default()
    }
}

fn preview_code_line_limit(preview_rows_visible: usize) -> usize {
    if preview_rows_visible == 0 {
        return default_code_preview_line_limit();
    }
    preview_rows_visible.saturating_mul(3).clamp(
        MIN_DYNAMIC_CODE_PREVIEW_LINE_LIMIT,
        default_code_preview_line_limit(),
    )
}
