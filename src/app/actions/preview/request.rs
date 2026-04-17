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
        // Use the initial (partial) render limit for the normal selection preview,
        // so the first render is fast. Prefetch paths call build_preview_request too
        // but we want them full — they pass code_line_limit as the render limit,
        // which happens to equal code_line_limit and therefore produces a complete render.
        let code_render_limit =
            initial_code_render_limit(self.input.frame_state.preview_rows_visible)
                .min(code_line_limit);
        PreviewRequest {
            token: self.preview.state.token,
            entry,
            variant,
            code_line_limit,
            code_render_limit,
            priority,
            work_class,
            ffprobe_available: self.ffprobe_available(),
            ffmpeg_available: self.terminal_image_overlay_available()
                && self.media_ffmpeg_available(),
        }
    }

    /// Build a preview request with `code_render_limit = code_line_limit` (full render).
    /// Used for prefetch and other paths that should always produce complete previews.
    pub(in crate::app) fn build_full_preview_request(
        &mut self,
        entry: Entry,
        variant: PreviewRequestOptions,
        priority: PreviewPriority,
        work_class: crate::preview::PreviewWorkClass,
    ) -> PreviewRequest {
        let code_line_limit = self.preview_code_line_limit_for_entry(&entry);
        PreviewRequest {
            token: self.preview.state.token,
            entry,
            variant,
            code_line_limit,
            code_render_limit: code_line_limit,
            priority,
            work_class,
            ffprobe_available: self.ffprobe_available(),
            ffmpeg_available: self.terminal_image_overlay_available()
                && self.media_ffmpeg_available(),
        }
    }

    /// Build an extension request for the currently-displayed partial preview.
    ///
    /// Returns `None` when:
    /// - The preview is already complete (no partial render in flight).
    /// - Another extension is already in flight for this entry.
    /// - There is no selected entry.
    pub(in crate::app) fn build_code_preview_extension_request(
        &mut self,
        entry: Entry,
        variant: PreviewRequestOptions,
        priority: PreviewPriority,
    ) -> Option<PreviewRequest> {
        if !self.preview.state.content.is_incrementally_partial() {
            return None;
        }
        if self.preview.state.incremental_render_in_flight {
            return None;
        }
        let code_line_limit = self.preview_code_line_limit_for_entry(&entry);
        let work_class = crate::preview::preview_work_class(&entry, &variant);
        Some(PreviewRequest {
            token: self.preview.state.token,
            entry,
            variant,
            code_line_limit,
            // Full render for the extension pass.
            code_render_limit: code_line_limit,
            priority,
            work_class,
            ffprobe_available: self.ffprobe_available(),
            ffmpeg_available: self.terminal_image_overlay_available()
                && self.media_ffmpeg_available(),
        })
    }

    pub(in crate::app) fn current_preview_request_options(&self) -> PreviewRequestOptions {
        self.comic_preview_request_options()
            .or_else(|| self.epub_preview_request_options())
            .unwrap_or_default()
    }

    pub(in crate::app) fn preview_code_line_limit_for_entry(&self, entry: &Entry) -> usize {
        self.preview_code_line_limit_for_entry_with_rows(
            entry,
            self.input.frame_state.preview_rows_visible,
        )
    }

    pub(in crate::app) fn preview_code_line_limit_for_entry_with_rows(
        &self,
        entry: &Entry,
        preview_rows_visible: usize,
    ) -> usize {
        let facts = crate::file_info::inspect_entry_cached(entry);
        if facts.preview.kind == crate::file_info::PreviewKind::Source
            && facts.preview.structured_format.is_none()
        {
            return preview_code_line_limit(preview_rows_visible);
        }
        default_code_preview_line_limit()
    }

    pub(in crate::app) fn preview_request_options_for_entry(
        &self,
        entry: &Entry,
    ) -> PreviewRequestOptions {
        self.comic_preview_request_options_for_entry(entry)
            .or_else(|| self.epub_preview_request_options_for_entry(entry))
            .unwrap_or_default()
    }
}

fn preview_code_line_limit(_preview_rows_visible: usize) -> usize {
    default_code_preview_line_limit()
}

/// Compute the initial (first-pass) render limit for incremental preview.
///
/// We render `rows × 2` lines so the first paint covers the full visible area
/// with some margin.  The result is clamped to
/// `[MIN_DYNAMIC_CODE_PREVIEW_LINE_LIMIT, default_code_preview_line_limit()]`
/// so we never render more than the full limit or fewer than the minimum.
fn initial_code_render_limit(preview_rows_visible: usize) -> usize {
    if preview_rows_visible == 0 {
        return default_code_preview_line_limit();
    }
    preview_rows_visible.saturating_mul(2).clamp(
        MIN_DYNAMIC_CODE_PREVIEW_LINE_LIMIT,
        default_code_preview_line_limit(),
    )
}
