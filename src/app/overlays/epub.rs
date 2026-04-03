use super::super::*;
use crate::file_info::{self, DocumentFormat};
use crate::preview::preview_work_class;
use std::{
    path::PathBuf,
    time::{Instant, SystemTime},
};

const EPUB_SECTION_PREFETCH_OFFSETS: [isize; 3] = [1, 2, -1];

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct EpubPreviewState {
    session: Option<EpubSession>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EpubSession {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    current_section: usize,
    total_sections: Option<usize>,
}

impl App {
    pub(in crate::app) fn sync_epub_preview_selection(&mut self) {
        let Some(entry) = self.selected_entry() else {
            self.preview.epub.session = None;
            return;
        };
        if !is_epub_entry(entry) {
            self.preview.epub.session = None;
            return;
        }

        let keep_session = self.preview.epub.session.as_ref().is_some_and(|session| {
            session.path == entry.path
                && session.size == entry.size
                && session.modified == entry.modified
        });
        if keep_session {
            return;
        }

        self.preview.epub.session = Some(EpubSession {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
            current_section: 0,
            total_sections: self.cached_epub_section_count(entry),
        });
    }

    pub(in crate::app) fn epub_preview_request_options(
        &self,
    ) -> Option<preview::PreviewRequestOptions> {
        self.preview
            .epub
            .session
            .as_ref()
            .map(|session| preview::PreviewRequestOptions::EpubSection(session.current_section))
    }

    pub(in crate::app) fn epub_preview_request_options_for_entry(
        &self,
        entry: &Entry,
    ) -> Option<preview::PreviewRequestOptions> {
        is_epub_entry(entry).then_some(preview::PreviewRequestOptions::EpubSection(0))
    }

    pub(in crate::app) fn epub_preview_wheel_capture_active(&self) -> bool {
        self.preview.epub.session.is_some()
    }

    pub(in crate::app) fn epub_preview_session_path(&self) -> Option<&std::path::Path> {
        self.preview.epub.session.as_ref().map(|s| s.path.as_path())
    }

    pub(in crate::app) fn apply_current_epub_preview_metadata(&mut self) {
        let Some((path, size, modified)) = self
            .selected_entry()
            .map(|entry| (entry.path.clone(), entry.size, entry.modified))
        else {
            return;
        };
        let Some(session) = self.preview.epub.session.as_mut() else {
            return;
        };
        if session.path != path || session.size != size || session.modified != modified {
            return;
        }

        if let Some(total_sections) = self.preview.state.content.ebook_section_count {
            session.total_sections = Some(total_sections);
        }
        if let Some(section_index) = self.preview.state.content.ebook_section_index {
            session.current_section = section_index;
        }
    }

    pub(in crate::app) fn apply_current_epub_loading_navigation(
        &self,
        preview: preview::PreviewContent,
    ) -> preview::PreviewContent {
        let Some(session) = self.preview.epub.session.as_ref() else {
            return preview;
        };
        let Some(total_sections) = session.total_sections else {
            return preview;
        };
        if preview.ebook_section_count.is_some() {
            return preview;
        }

        preview.with_ebook_section(session.current_section, total_sections, None)
    }

    pub(in crate::app) fn step_epub_section(&mut self, delta: isize) -> bool {
        let Some(session) = self.preview.epub.session.as_mut() else {
            return false;
        };
        let total_sections = session
            .total_sections
            .or(self.preview.state.content.ebook_section_count)
            .unwrap_or(0);
        if total_sections == 0 {
            return false;
        }

        let previous = session.current_section;
        let next = if delta.is_negative() {
            previous.saturating_sub(delta.unsigned_abs())
        } else {
            previous.saturating_add(delta as usize)
        };
        session.current_section = next.min(total_sections.saturating_sub(1));
        if session.current_section == previous {
            return false;
        }

        self.preview.state.deferred_refresh_at = Some(Instant::now());
        self.refresh_preview();
        true
    }

    pub(in crate::app) fn epub_prefetch_section_indices(&self) -> Vec<usize> {
        let Some(session) = self.preview.epub.session.as_ref() else {
            return Vec::new();
        };
        let total_sections = session
            .total_sections
            .or(self.preview.state.content.ebook_section_count)
            .unwrap_or(0);
        if total_sections == 0 {
            return Vec::new();
        }

        EPUB_SECTION_PREFETCH_OFFSETS
            .into_iter()
            .filter_map(|offset| {
                let section = if offset.is_negative() {
                    session.current_section.checked_sub(offset.unsigned_abs())?
                } else {
                    session.current_section.checked_add(offset as usize)?
                };
                (section < total_sections && section != session.current_section).then_some(section)
            })
            .collect()
    }

    pub(in crate::app) fn prefetch_nearby_epub_sections(&mut self) {
        let Some(entry) = self.selected_entry().cloned() else {
            return;
        };
        if !is_epub_entry(&entry) {
            return;
        }

        for section in self.epub_prefetch_section_indices() {
            let variant = preview::PreviewRequestOptions::EpubSection(section);
            if self.cached_preview_for(&entry, &variant).is_some() {
                continue;
            }

            let request = self.build_preview_request(
                entry.clone(),
                variant.clone(),
                PreviewPriority::Low,
                preview_work_class(&entry, &variant),
            );
            let _ = self.jobs.scheduler.submit_preview(request);
        }
    }

    pub(in crate::app) fn nearby_epub_preview_visual_overlay_requests(
        &self,
    ) -> Vec<crate::app::overlays::images::StaticImageOverlayRequest> {
        let Some(entry) = self.selected_entry() else {
            return Vec::new();
        };
        if !is_epub_entry(entry) {
            return Vec::new();
        }
        let Some(area) = self.input.frame_state.preview_media_area else {
            return Vec::new();
        };

        self.epub_prefetch_section_indices()
            .into_iter()
            .filter_map(|section| {
                let variant = preview::PreviewRequestOptions::EpubSection(section);
                let cached = self.cached_preview_for(entry, &variant)?;
                let visual = cached.preview_visual.as_ref()?;
                (cached.kind == preview::PreviewKind::Document
                    && visual.kind == preview::PreviewVisualKind::PageImage)
                    .then(|| {
                        self.preview_visual_overlay_request_for_visual(cached.kind, visual, area)
                    })
            })
            .collect()
    }

    fn cached_epub_section_count(&self, entry: &Entry) -> Option<usize> {
        self.preview
            .state
            .result_cache
            .iter()
            .find_map(|(key, cached)| {
                (key.path == entry.path
                    && cached.size == entry.size
                    && cached.modified == entry.modified)
                    .then_some(cached.preview.ebook_section_count)
                    .flatten()
            })
    }

    #[cfg(test)]
    pub(in crate::app) fn has_cached_epub_preview_section(
        &self,
        path: &std::path::Path,
        section: usize,
    ) -> bool {
        self.preview
            .state
            .result_cache
            .contains_key(&PreviewCacheKey {
                path: path.to_path_buf(),
                variant: preview::PreviewRequestOptions::EpubSection(section),
                code_line_limit: preview::default_code_preview_line_limit(),
                code_render_limit: preview::default_code_preview_line_limit(),
            })
    }
}

fn is_epub_entry(entry: &Entry) -> bool {
    file_info::inspect_path_cached(&entry.path, entry.kind, entry.size, entry.modified)
        .preview
        .document_format
        == Some(DocumentFormat::Epub)
}
