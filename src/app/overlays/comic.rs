use super::super::*;
use crate::preview::preview_work_class;
use std::{
    path::PathBuf,
    time::{Instant, SystemTime},
};

const COMIC_PAGE_PREFETCH_OFFSETS: [isize; 3] = [1, 2, -1];
const COMIC_ENTRY_PREFETCH_OFFSETS: [isize; 2] = [1, -1];

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct ComicPreviewState {
    session: Option<ComicSession>,
    /// Path of the comic file whose page image is currently displayed in the
    /// inline overlay.  Set when a page image is rendered so we can decide
    /// whether to keep or clear the stale overlay when the selection changes.
    displayed_page_source: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ComicSession {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    current_page: usize,
    total_pages: Option<usize>,
}

impl App {
    pub(in crate::app) fn sync_comic_preview_selection(&mut self) {
        let Some(entry) = self.selected_entry() else {
            self.comic_preview.session = None;
            return;
        };
        if !is_comic_entry(entry) {
            self.comic_preview.session = None;
            return;
        }

        let keep_session = self.comic_preview.session.as_ref().is_some_and(|session| {
            session.path == entry.path
                && session.size == entry.size
                && session.modified == entry.modified
        });
        if keep_session {
            return;
        }

        self.comic_preview.session = Some(ComicSession {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
            current_page: 0,
            total_pages: self.cached_comic_page_count(entry),
        });
    }

    pub(in crate::app) fn comic_preview_request_options(
        &self,
    ) -> Option<preview::PreviewRequestOptions> {
        self.comic_preview
            .session
            .as_ref()
            .map(|session| preview::PreviewRequestOptions::ComicPage(session.current_page))
    }

    pub(in crate::app) fn comic_preview_request_options_for_entry(
        &self,
        entry: &Entry,
    ) -> Option<preview::PreviewRequestOptions> {
        is_comic_entry(entry).then_some(preview::PreviewRequestOptions::ComicPage(0))
    }

    pub(in crate::app) fn comic_preview_wheel_capture_active(&self) -> bool {
        self.comic_preview.session.is_some()
    }

    /// Called just after a page image (comic or fixed-layout EPUB) is rendered
    /// to the inline overlay.  Records the source file path so
    /// `displayed_page_image_belongs_to_current_session` can later decide
    /// whether to keep or clear the stale overlay on navigation.
    pub(in crate::app) fn record_comic_page_image_displayed(&mut self) {
        let is_page_image = self
            .preview_state
            .content
            .preview_visual
            .as_ref()
            .is_some_and(|v| v.kind == crate::preview::PreviewVisualKind::PageImage);
        if is_page_image {
            // Use whichever page-session is currently active.  Comics have a
            // comic_preview session; fixed-layout EPUBs have an epub_preview
            // session.  Both are updated in sync_*_preview_selection() which
            // runs inside refresh_preview(), so during a deferred navigation
            // they still point to the file whose page is actually on screen.
            self.comic_preview.displayed_page_source = self
                .comic_preview
                .session
                .as_ref()
                .map(|s| s.path.clone())
                .or_else(|| self.epub_preview_session_path().map(|p| p.to_path_buf()));
        }
    }

    /// Returns `true` if the page image currently displayed in the inline
    /// overlay was rendered for the same source file as the active session.
    /// Covers both comic sessions and fixed-layout EPUB sessions.
    /// Both being `None` is treated as matching (no session info available).
    pub(in crate::app) fn displayed_comic_page_belongs_to_current_session(&self) -> bool {
        let current_source = self
            .comic_preview
            .session
            .as_ref()
            .map(|s| s.path.as_path())
            .or_else(|| self.epub_preview_session_path());
        self.comic_preview.displayed_page_source.as_deref() == current_source
    }

    pub(in crate::app) fn apply_current_comic_preview_metadata(&mut self) {
        let Some((path, size, modified)) = self
            .selected_entry()
            .map(|entry| (entry.path.clone(), entry.size, entry.modified))
        else {
            return;
        };
        let Some(session) = self.comic_preview.session.as_mut() else {
            return;
        };
        if session.path != path || session.size != size || session.modified != modified {
            return;
        }

        let Some(position) = self.preview_state.content.navigation_position.as_ref() else {
            return;
        };
        if position.label != "Page" {
            return;
        }

        session.total_pages = Some(position.count);
        session.current_page = position.index;
    }

    pub(in crate::app) fn apply_current_comic_loading_navigation(
        &self,
        preview: preview::PreviewContent,
    ) -> preview::PreviewContent {
        let Some(session) = self.comic_preview.session.as_ref() else {
            return preview;
        };
        let Some(total_pages) = session.total_pages else {
            return preview;
        };
        if preview.kind != preview::PreviewKind::Comic {
            return preview;
        }

        preview.with_navigation_position("Page", session.current_page, total_pages, None)
    }

    pub(in crate::app) fn step_comic_page(&mut self, delta: isize) -> bool {
        self.step_comic_page_with_preview_mode(delta, PreviewRefreshMode::Immediate)
    }

    pub(in crate::app) fn step_comic_page_with_preview_mode(
        &mut self,
        delta: isize,
        preview_mode: PreviewRefreshMode,
    ) -> bool {
        let Some(session) = self.comic_preview.session.as_mut() else {
            return false;
        };
        let total_pages = session
            .total_pages
            .or(self
                .preview_state
                .content
                .navigation_position
                .as_ref()
                .filter(|position| position.label == "Page")
                .map(|position| position.count))
            .unwrap_or(0);
        if total_pages == 0 {
            return false;
        }

        let previous = session.current_page;
        let next = if delta.is_negative() {
            previous.saturating_sub(delta.unsigned_abs())
        } else {
            previous.saturating_add(delta as usize)
        };
        session.current_page = next.min(total_pages.saturating_sub(1));
        if session.current_page == previous {
            return false;
        }

        match preview_mode {
            PreviewRefreshMode::Immediate => {
                self.image_preview.selection_activation_delay = std::time::Duration::ZERO;
                self.preview_state.deferred_refresh_at = Some(Instant::now());
                self.refresh_preview();
            }
            PreviewRefreshMode::Deferred => {
                self.last_selection_change_at = Instant::now();
                self.image_preview.selection_activation_delay = IMAGE_SELECTION_ACTIVATION_DELAY;
                self.preview_state.deferred_refresh_at =
                    Some(Instant::now() + HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY);
                if let Some(position) = self.preview_state.content.navigation_position.as_mut()
                    && position.label == "Page"
                {
                    position.index = session.current_page;
                }
                self.preview_state.scroll = 0;
                self.preview_state.horizontal_scroll = 0;
                self.sync_preview_scroll();
            }
        }
        true
    }

    pub(in crate::app) fn comic_prefetch_page_indices(&self) -> Vec<usize> {
        let Some(session) = self.comic_preview.session.as_ref() else {
            return Vec::new();
        };
        let total_pages = session
            .total_pages
            .or(self
                .preview_state
                .content
                .navigation_position
                .as_ref()
                .filter(|position| position.label == "Page")
                .map(|position| position.count))
            .unwrap_or(0);
        if total_pages == 0 {
            return Vec::new();
        }

        COMIC_PAGE_PREFETCH_OFFSETS
            .into_iter()
            .filter_map(|offset| {
                let page = if offset.is_negative() {
                    session.current_page.checked_sub(offset.unsigned_abs())?
                } else {
                    session.current_page.checked_add(offset as usize)?
                };
                (page < total_pages && page != session.current_page).then_some(page)
            })
            .collect()
    }

    pub(in crate::app) fn prefetch_nearby_comic_pages(&mut self) {
        let Some(entry) = self.selected_entry().cloned() else {
            return;
        };
        if !is_comic_entry(&entry) {
            return;
        }

        for page in self.comic_prefetch_page_indices() {
            let variant = preview::PreviewRequestOptions::ComicPage(page);
            if self.cached_preview_for(&entry, &variant).is_some() {
                continue;
            }

            let request = self.build_preview_request(
                entry.clone(),
                variant.clone(),
                PreviewPriority::Low,
                preview_work_class(&entry, &variant),
            );
            let _ = self.scheduler.submit_preview(request);
        }
    }

    pub(in crate::app) fn prefetch_nearby_comic_entries(&mut self) {
        let Some(current_entry) = self.selected_entry() else {
            return;
        };
        if !is_comic_entry(current_entry) {
            return;
        }
        let current_variant = self.current_preview_request_options();
        if self
            .cached_preview_for(current_entry, &current_variant)
            .is_none()
        {
            return;
        }

        for entry in self.nearby_comic_entry_candidates() {
            let variant = preview::PreviewRequestOptions::ComicPage(0);
            if self.cached_preview_for(&entry, &variant).is_some() {
                continue;
            }

            let request = self.build_preview_request(
                entry.clone(),
                variant.clone(),
                PreviewPriority::Low,
                preview_work_class(&entry, &variant),
            );
            let _ = self.scheduler.submit_preview(request);
        }
    }

    pub(in crate::app) fn nearby_comic_preview_visual_overlay_requests(
        &self,
    ) -> Vec<crate::app::overlays::images::StaticImageOverlayRequest> {
        let Some(entry) = self.selected_entry() else {
            return Vec::new();
        };
        if !is_comic_entry(entry) {
            return Vec::new();
        }
        let Some(area) = self.frame_state.preview_media_area else {
            return Vec::new();
        };

        self.comic_prefetch_page_indices()
            .into_iter()
            .filter_map(|page| {
                let variant = preview::PreviewRequestOptions::ComicPage(page);
                let cached = self.cached_preview_for(entry, &variant)?;
                let visual = cached.preview_visual.as_ref()?;
                (cached.kind == preview::PreviewKind::Comic
                    && visual.kind == preview::PreviewVisualKind::PageImage)
                    .then(|| {
                        self.preview_visual_overlay_request_for_visual(cached.kind, visual, area)
                    })
            })
            .collect()
    }

    pub(in crate::app) fn nearby_comic_entry_preview_visual_overlay_requests(
        &self,
    ) -> Vec<crate::app::overlays::images::StaticImageOverlayRequest> {
        let Some(area) = self.frame_state.preview_media_area else {
            return Vec::new();
        };

        self.nearby_comic_entry_candidates()
            .into_iter()
            .filter_map(|entry| {
                let variant = preview::PreviewRequestOptions::ComicPage(0);
                let cached = self.cached_preview_for(&entry, &variant)?;
                let visual = cached.preview_visual.as_ref()?;
                (cached.kind == preview::PreviewKind::Comic
                    && visual.kind == preview::PreviewVisualKind::PageImage)
                    .then(|| {
                        self.preview_visual_overlay_request_for_visual(cached.kind, visual, area)
                    })
            })
            .collect()
    }

    pub(in crate::app) fn refreshes_image_preloads_for_nearby_comic_entry_preview(
        &self,
        entry: &Entry,
        variant: &preview::PreviewRequestOptions,
    ) -> bool {
        variant == &preview::PreviewRequestOptions::ComicPage(0)
            && self
                .nearby_comic_entry_candidates()
                .into_iter()
                .any(|candidate| {
                    candidate.path == entry.path
                        && candidate.size == entry.size
                        && candidate.modified == entry.modified
                })
    }

    fn cached_comic_page_count(&self, entry: &Entry) -> Option<usize> {
        self.preview_state
            .result_cache
            .iter()
            .find_map(|(key, cached)| {
                (key.path == entry.path
                    && cached.size == entry.size
                    && cached.modified == entry.modified)
                    .then_some(cached.preview.navigation_position.as_ref())
                    .flatten()
                    .filter(|position| position.label == "Page")
                    .map(|position| position.count)
            })
    }

    #[cfg(test)]
    pub(in crate::app) fn has_cached_comic_preview_page(
        &self,
        path: &std::path::Path,
        page: usize,
    ) -> bool {
        self.preview_state
            .result_cache
            .contains_key(&PreviewCacheKey {
                path: path.to_path_buf(),
                variant: preview::PreviewRequestOptions::ComicPage(page),
                code_line_limit: preview::default_code_preview_line_limit(),
                code_render_limit: preview::default_code_preview_line_limit(),
            })
    }

    fn nearby_comic_entry_candidates(&self) -> Vec<Entry> {
        let Some(entry) = self.selected_entry() else {
            return Vec::new();
        };
        if !is_comic_entry(entry) {
            return Vec::new();
        }

        COMIC_ENTRY_PREFETCH_OFFSETS
            .into_iter()
            .filter_map(|offset| {
                let target = self.selected as isize + offset;
                (target >= 0)
                    .then_some(target as usize)
                    .and_then(|index| self.entries.get(index))
                    .filter(|candidate| is_comic_entry(candidate))
                    .cloned()
            })
            .collect()
    }
}

fn is_comic_entry(entry: &Entry) -> bool {
    entry
        .path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cbz") || ext.eq_ignore_ascii_case("cbr"))
}
