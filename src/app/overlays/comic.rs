use super::super::*;
use crate::preview::preview_work_class;
use std::{
    path::PathBuf,
    time::{Instant, SystemTime},
};

const COMIC_PAGE_PREFETCH_OFFSETS: [isize; 3] = [1, 2, -1];

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct ComicPreviewState {
    session: Option<ComicSession>,
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

            let _ = self.scheduler.submit_preview(PreviewRequest {
                token: self.preview_state.token,
                entry: entry.clone(),
                work_class: preview_work_class(&entry, &variant),
                variant,
                code_line_limit: self.preview_code_line_limit_for_entry(&entry),
                priority: PreviewPriority::Low,
            });
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
            })
    }
}

fn is_comic_entry(entry: &Entry) -> bool {
    entry
        .path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cbz") || ext.eq_ignore_ascii_case("cbr"))
}
