use super::super::App;
use crate::background_jobs::job_results::{
    DirectoryStatsBuild, ImagePrepareBuild, PdfProbeBuild, PdfRenderBuild, PreviewBuild,
    PreviewLineCountBuild,
};
use crate::filesystem::Entry;
use crate::preview::{self, PreviewLoadState};

impl App {
    pub(super) fn apply_directory_stats_job_result(&mut self, build: DirectoryStatsBuild) -> bool {
        self.apply_preview_directory_stats_result(build.token, &build.path, build.result)
    }

    pub(super) fn apply_preview_line_count_job_result(
        &mut self,
        build: PreviewLineCountBuild,
    ) -> bool {
        self.apply_preview_line_count_result(
            &build.path,
            build.size,
            build.modified,
            build.total_lines,
        )
    }

    pub(super) fn apply_pdf_probe_job_result(&mut self, build: PdfProbeBuild) -> bool {
        self.apply_pdf_probe_build(build)
    }

    pub(super) fn apply_pdf_render_job_result(&mut self, build: PdfRenderBuild) -> bool {
        self.apply_pdf_render_build(build)
    }

    pub(super) fn apply_image_prepare_job_result(&mut self, build: ImagePrepareBuild) -> bool {
        self.apply_image_prepare_build(build)
    }

    fn refresh_static_image_preloads_for_cached_preview_visual(
        &mut self,
        build_entry: &Entry,
        build_variant: &preview::PreviewRequestOptions,
        build_visual_kind: Option<preview::PreviewVisualKind>,
        is_current_entry: bool,
    ) {
        let Some(build_visual_kind) = build_visual_kind else {
            return;
        };

        let should_refresh = match build_visual_kind {
            preview::PreviewVisualKind::PageImage => {
                is_current_entry
                    || self.refreshes_image_preloads_for_nearby_comic_entry_preview(
                        build_entry,
                        build_variant,
                    )
                    || self.refreshes_image_preloads_for_nearby_epub_entry_preview(
                        build_entry,
                        build_variant,
                    )
            }
            preview::PreviewVisualKind::Cover => {
                is_current_entry
                    || self.refreshes_image_preloads_for_nearby_audio_preview(
                        build_entry,
                        build_variant,
                    )
            }
        };
        if should_refresh {
            self.refresh_static_image_preloads();
        }
    }

    pub(super) fn apply_preview_job_result(&mut self, build: Box<PreviewBuild>) -> bool {
        self.preview.state.remember_preview(
            &build.entry,
            &build.variant,
            build.code_line_limit,
            build.code_render_limit,
            build.ffmpeg_available,
            &build.result,
        );
        let build_is_comic = build.result.kind == preview::PreviewKind::Comic;
        let build_is_epub_section = matches!(
            build.variant,
            preview::PreviewRequestOptions::EpubSection(_)
        );
        let build_visual_kind = build
            .result
            .preview_visual
            .as_ref()
            .map(|visual| visual.kind);
        let is_current_entry = self
            .active_preview_entry()
            .map(|entry| {
                entry.path == build.entry.path
                    && entry.modified == build.entry.modified
                    && entry.size == build.entry.size
            })
            .unwrap_or(false);
        let is_current_variant = build.variant == self.current_preview_request_options();
        if build.token != self.preview.state.token
            || !is_current_entry
            || !is_current_variant
            || build.code_line_limit != self.preview_code_line_limit_for_entry(&build.entry)
        {
            // For comic results that match the current entry and variant but arrived with a stale
            // token, apply them immediately if we are still showing a placeholder. The rendered
            // page list is deterministic for a given path and page index, so a token skew does not
            // indicate wrong content. This rescues the race where a rapid navigation refresh bumps
            // the token after the job was submitted and before the replacement job finishes.
            let can_rescue_stale_comic = build_is_comic
                && is_current_entry
                && is_current_variant
                && build.code_line_limit == self.preview_code_line_limit_for_entry(&build.entry)
                && matches!(
                    &self.preview.state.load_state,
                    Some(PreviewLoadState::Placeholder(path) | PreviewLoadState::Refreshing(path))
                        if path == &build.entry.path
                );
            if !can_rescue_stale_comic {
                self.refresh_static_image_preloads_for_cached_preview_visual(
                    &build.entry,
                    &build.variant,
                    build_visual_kind,
                    is_current_entry,
                );
                // Clear the in-flight flag when this stale drop belongs to our outstanding
                // extension job, preventing stuck state after navigating away mid-extension.
                if self.preview.state.incremental_render_path.as_deref()
                    == Some(build.entry.path.as_path())
                {
                    self.preview.state.incremental_render_in_flight = false;
                    self.preview.state.incremental_render_path = None;
                }
                self.preview.state.metrics.stale_results_dropped += 1;
                return false;
            }
        }

        // A complete extension result replaces the partial preview without resetting scroll.
        let is_extension_result = build.result.incremental_render_limit.is_none()
            && self.preview.state.content.is_incrementally_partial()
            && is_current_entry
            && is_current_variant;

        self.preview.state.content = build.result;
        if self.preview.state.content.kind != preview::PreviewKind::Directory {
            self.clear_preview_directory_stats();
        }
        self.preview.state.load_state = None;
        self.apply_current_comic_preview_metadata();
        self.apply_current_epub_preview_metadata();
        self.sync_current_preview_line_count();

        if is_extension_result {
            self.preview.state.incremental_render_in_flight = false;
            self.preview.state.incremental_render_path = None;
            self.sync_preview_scroll();
        } else {
            self.preview.state.scroll = 0;
            self.preview.state.horizontal_scroll = 0;
            self.sync_preview_scroll();
        }

        if build_visual_kind.is_some() {
            self.refresh_static_image_preloads();
        }
        if build_is_comic || build_is_epub_section || is_current_entry {
            self.prefetch_nearby_audio_previews();
            self.schedule_preview_prefetch();
        }
        self.preview.state.metrics.applied_results += 1;
        true
    }
}
