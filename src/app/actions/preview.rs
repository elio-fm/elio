use super::*;
use crate::preview::{
    MIN_DYNAMIC_CODE_PREVIEW_LINE_LIMIT, PreviewContent, PreviewKind, PreviewLineCoverage,
    PreviewRequestOptions, PreviewWorkClass, default_code_preview_line_limit, loading_preview_for,
    preview_work_class, should_build_preview_in_background,
};
use std::sync::Arc;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const FITTED_HEADER_SEPARATOR: &str = " • ";
const FREEFORM_COMPACT_WIDTH: usize = 18;
const HEADER_SCORE_NAVIGATION: u32 = 12_000;
const HEADER_SCORE_STATUS: u32 = 9_000;
const HEADER_SCORE_DETAIL: u32 = 8_000;
const HEADER_SCORE_LINE_COVERAGE: u32 = 7_000;
const HEADER_SCORE_TITLE: u32 = 3_500;
const HEADER_SCORE_CONTEXT: u32 = 2_000;
const HEADER_SCORE_AUXILIARY: u32 = 500;

#[derive(Clone, Debug)]
struct PreviewHeaderSegment {
    variants: Vec<PreviewHeaderVariant>,
}

#[derive(Clone, Debug)]
struct PreviewHeaderVariant {
    text: Option<String>,
    score: u32,
}

impl PreviewHeaderSegment {
    fn new(weight: u32, full: String, compact: Option<String>) -> Self {
        let mut variants = vec![PreviewHeaderVariant {
            text: Some(full.clone()),
            score: weight + 20,
        }];
        if let Some(compact) = compact
            && compact != full
        {
            variants.push(PreviewHeaderVariant {
                text: Some(compact),
                score: weight + 10,
            });
        }
        variants.push(PreviewHeaderVariant {
            text: None,
            score: 0,
        });
        Self { variants }
    }
}

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
        fit_preview_header_segments(&segments, available_width)
    }

    pub(in crate::app) fn current_preview_request_options(&self) -> PreviewRequestOptions {
        self.comic_preview_request_options()
            .or_else(|| self.epub_preview_request_options())
            .unwrap_or_default()
    }

    pub(in crate::app) fn refresh_preview(&mut self) {
        self.preview_state.deferred_refresh_at = None;
        self.preview_state.prefetch_ready_at = None;
        self.sync_comic_preview_selection();
        self.sync_epub_preview_selection();
        self.sync_pdf_preview_selection();
        self.sync_image_preview_selection_activation();
        self.preview_state.token = self.preview_state.token.wrapping_add(1);
        let preview_options = self.current_preview_request_options();
        self.preview_state.content = match self.selected_entry().cloned() {
            Some(entry) if self.should_defer_static_image_preview(&entry) => {
                self.preview_state.load_state = None;
                PreviewContent::new(PreviewKind::Image, Vec::new()).with_detail(
                    self.static_image_preview_detail(&entry)
                        .unwrap_or("Image preview"),
                )
            }
            Some(entry) if self.should_defer_pdf_document_preview(&entry) => {
                self.preview_state.load_state = None;
                self.cached_preview_for(&entry, &preview_options)
                    .or_else(|| self.stale_cached_preview_for(&entry, &preview_options))
                    .unwrap_or_else(|| {
                        PreviewContent::new(PreviewKind::Document, Vec::new())
                            .with_detail("PDF document")
                    })
            }
            Some(entry) => {
                if let Some(preview) = self.cached_preview_for(&entry, &preview_options) {
                    self.preview_state.metrics.cache_hits += 1;
                    self.preview_state.load_state = None;
                    preview
                } else if let Some(stale_preview) =
                    self.stale_cached_preview_for(&entry, &preview_options)
                {
                    self.preview_state.metrics.cache_misses += 1;
                    let loading_path = entry.path.clone();
                    let work_class = preview_work_class(&entry, &preview_options);
                    let code_line_limit = self.preview_code_line_limit_for_entry(&entry);
                    let request = PreviewRequest {
                        token: self.preview_state.token,
                        entry,
                        variant: preview_options.clone(),
                        code_line_limit,
                        priority: PreviewPriority::High,
                        work_class,
                    };
                    if !self.scheduler.submit_preview(request) {
                        self.preview_state.load_state = None;
                        stale_preview.with_status_note("Refresh unavailable")
                    } else {
                        self.preview_state.load_state =
                            Some(PreviewLoadState::Refreshing(loading_path));
                        stale_preview.with_status_note("Refreshing in background")
                    }
                } else {
                    self.preview_state.metrics.cache_misses += 1;
                    let placeholder = self.apply_current_epub_loading_navigation(
                        self.apply_current_comic_loading_navigation(loading_preview_for(
                            &entry,
                            &preview_options,
                        )),
                    );
                    let loading_path = entry.path.clone();
                    let work_class = preview_work_class(&entry, &preview_options);
                    let code_line_limit = self.preview_code_line_limit_for_entry(&entry);
                    let request = PreviewRequest {
                        token: self.preview_state.token,
                        entry,
                        variant: preview_options.clone(),
                        code_line_limit,
                        priority: PreviewPriority::High,
                        work_class,
                    };
                    if !self.scheduler.submit_preview(request) {
                        self.preview_state.load_state = None;
                        PreviewContent::placeholder("Preview worker unavailable")
                    } else {
                        self.preview_state.load_state =
                            Some(PreviewLoadState::Placeholder(loading_path));
                        placeholder
                    }
                }
            }
            None => {
                self.preview_state.load_state = None;
                PreviewContent::placeholder("No selection")
            }
        };
        self.apply_current_comic_preview_metadata();
        self.apply_current_epub_preview_metadata();
        self.sync_current_preview_line_count();
        self.preview_state.scroll = 0;
        self.preview_state.horizontal_scroll = 0;
        self.sync_preview_scroll();
        self.refresh_static_image_preloads();
        self.schedule_preview_prefetch();
    }

    pub(crate) fn process_preview_refresh_timers(&mut self) -> bool {
        let Some(deadline) = self.preview_state.deferred_refresh_at else {
            return false;
        };
        if Instant::now() < deadline {
            return false;
        }
        self.refresh_preview();
        true
    }

    pub(crate) fn pending_preview_refresh_timer(&self) -> Option<std::time::Duration> {
        self.preview_state
            .deferred_refresh_at
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    pub(crate) fn process_preview_prefetch_timers(&mut self) -> bool {
        let Some(deadline) = self.preview_state.prefetch_ready_at else {
            return false;
        };
        if Instant::now() < deadline
            || self.preview_state.deferred_refresh_at.is_some()
            || self.browser_wheel_burst_active()
        {
            return false;
        }

        self.preview_state.prefetch_ready_at = None;
        self.prefetch_nearby_comic_pages();
        self.prefetch_nearby_epub_sections();
        self.prefetch_nearby_previews();
        false
    }

    pub(crate) fn pending_preview_prefetch_timer(&self) -> Option<std::time::Duration> {
        self.preview_state
            .prefetch_ready_at
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    pub(in crate::app) fn cached_preview_for(
        &self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
    ) -> Option<PreviewContent> {
        let cached = self.preview_state.result_cache.get(&PreviewCacheKey {
            path: entry.path.clone(),
            variant: variant.clone(),
            code_line_limit: self.preview_code_line_limit_for_entry(entry),
        })?;
        if cached.size == entry.size && cached.modified == entry.modified {
            Some(cached.preview.clone())
        } else {
            None
        }
    }

    fn stale_cached_preview_for(
        &self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
    ) -> Option<PreviewContent> {
        self.preview_state
            .result_cache
            .get(&PreviewCacheKey {
                path: entry.path.clone(),
                variant: variant.clone(),
                code_line_limit: self.preview_code_line_limit_for_entry(entry),
            })
            .map(|cached| cached.preview.clone())
    }

    #[cfg(test)]
    pub(in crate::app) fn cache_preview_result(
        &mut self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
        preview: &PreviewContent,
    ) {
        let code_line_limit = self.preview_code_line_limit_for_entry(entry);
        self.cache_preview_result_with_code_line_limit(entry, variant, code_line_limit, preview);
    }

    pub(in crate::app) fn cache_preview_result_with_code_line_limit(
        &mut self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
        code_line_limit: usize,
        preview: &PreviewContent,
    ) {
        let key = PreviewCacheKey {
            path: entry.path.clone(),
            variant: variant.clone(),
            code_line_limit,
        };
        self.preview_state.result_cache.insert(
            key.clone(),
            CachedPreview {
                size: entry.size,
                modified: entry.modified,
                preview: preview.clone(),
            },
        );
        self.preview_state
            .result_order
            .retain(|cached| cached != &key);
        self.preview_state.result_order.push_back(key);

        while self.preview_state.result_order.len() > PREVIEW_CACHE_LIMIT {
            if let Some(stale_key) = self.preview_state.result_order.pop_front() {
                self.preview_state.result_cache.remove(&stale_key);
            }
        }
    }

    pub(in crate::app) fn cache_preview_line_count(
        &mut self,
        path: PathBuf,
        size: u64,
        modified: Option<SystemTime>,
        total_lines: usize,
    ) {
        let key = PreviewLineCountKey {
            path,
            size,
            modified,
        };
        self.preview_state
            .line_count_cache
            .insert(key.clone(), total_lines.max(1));
        self.preview_state
            .line_count_order
            .retain(|cached| cached != &key);
        self.preview_state.line_count_order.push_back(key);

        while self.preview_state.line_count_order.len() > PREVIEW_LINE_COUNT_CACHE_LIMIT {
            if let Some(stale_key) = self.preview_state.line_count_order.pop_front() {
                self.preview_state.line_count_cache.remove(&stale_key);
            }
        }
    }

    pub(in crate::app) fn apply_preview_line_count_result(
        &mut self,
        path: &std::path::Path,
        size: u64,
        modified: Option<SystemTime>,
        total_lines: Option<usize>,
    ) -> bool {
        let key = PreviewLineCountKey {
            path: path.to_path_buf(),
            size,
            modified,
        };
        self.preview_state.pending_line_counts.remove(&key);
        let Some(total_lines) = total_lines else {
            let should_clear_pending = self.selected_entry().is_some_and(|entry| {
                entry.path == key.path && entry.size == key.size && entry.modified == key.modified
            });
            if should_clear_pending {
                self.preview_state
                    .content
                    .set_total_line_count_pending(false);
                return true;
            }
            return false;
        };
        self.cache_preview_line_count(key.path.clone(), key.size, key.modified, total_lines);

        let is_current_entry = self.selected_entry().is_some_and(|entry| {
            entry.path == key.path && entry.size == key.size && entry.modified == key.modified
        });
        if is_current_entry {
            self.preview_state
                .content
                .apply_total_line_count(total_lines);
            return true;
        }
        false
    }

    fn prefetch_nearby_previews(&mut self) {
        let mut queued = 0;
        for offset in [1isize, -1, 2, -2, 3, -3] {
            if queued >= PREVIEW_PREFETCH_LIMIT {
                break;
            }

            let target = self.selected as isize + offset;
            if target < 0 {
                continue;
            }
            let Some(entry) = self.entries.get(target as usize).cloned() else {
                continue;
            };
            let variant = self.preview_request_options_for_entry(&entry);
            let work_class = preview_work_class(&entry, &variant);
            if !should_build_preview_in_background(&entry)
                || work_class == PreviewWorkClass::Heavy
                || self.cached_preview_for(&entry, &variant).is_some()
            {
                continue;
            }

            let code_line_limit = self.preview_code_line_limit_for_entry(&entry);
            let request = PreviewRequest {
                token: self.preview_state.token,
                entry,
                variant,
                code_line_limit,
                priority: PreviewPriority::Low,
                work_class: PreviewWorkClass::Light,
            };
            if self.scheduler.submit_preview(request) {
                queued += 1;
            }
        }
    }

    pub(in crate::app) fn schedule_preview_prefetch(&mut self) {
        self.preview_state.prefetch_ready_at = self
            .selected_entry()
            .map(|_| Instant::now() + PREVIEW_PREFETCH_IDLE_DELAY);
    }

    fn preview_request_options_for_entry(&self, entry: &Entry) -> PreviewRequestOptions {
        self.comic_preview_request_options_for_entry(entry)
            .or_else(|| self.epub_preview_request_options_for_entry(entry))
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

    #[cfg(test)]
    pub(in crate::app) fn has_cached_preview_for_path(&self, path: &std::path::Path) -> bool {
        self.preview_state
            .result_cache
            .keys()
            .any(|key| key.path == path)
    }

    pub(in crate::app) fn sync_current_preview_line_count(&mut self) {
        let needs_total_line_count = self.preview_state.content.needs_total_line_count();
        let Some(entry) = self.selected_entry().cloned() else {
            return;
        };
        if !needs_total_line_count {
            self.preview_state
                .content
                .set_total_line_count_pending(false);
            return;
        }

        let key = PreviewLineCountKey {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
        };
        if let Some(total_lines) = self.preview_state.line_count_cache.get(&key).copied() {
            self.preview_state
                .content
                .apply_total_line_count(total_lines);
            return;
        }

        let pending = self.preview_state.pending_line_counts.contains(&key)
            || self
                .scheduler
                .submit_preview_line_count(PreviewLineCountRequest {
                    path: entry.path,
                    size: entry.size,
                    modified: entry.modified,
                });
        if pending {
            self.preview_state.pending_line_counts.insert(key);
        }
        self.preview_state
            .content
            .set_total_line_count_pending(pending);
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

impl App {
    fn preview_header_segments(&self, visible_rows: usize) -> Vec<PreviewHeaderSegment> {
        let mut segments = Vec::new();
        let content = &self.preview_state.content;

        if let Some(position) = content.navigation_position.as_ref() {
            let full = format!(
                "{} {}/{}",
                position.label,
                position.index + 1,
                position.count
            );
            let compact = Some(format!("{}/{}", position.index + 1, position.count));
            segments.push(PreviewHeaderSegment::new(
                HEADER_SCORE_NAVIGATION,
                full,
                compact,
            ));
        }

        if let Some(segment) = self.pdf_preview_header_segment() {
            segments.push(segment);
        }

        if let Some(detail) = content.detail.as_deref()
            && !detail.is_empty()
        {
            let detail = sanitize_terminal_text(detail);
            let header_detail =
                compact_preview_header_label(&detail).unwrap_or_else(|| detail.clone());
            segments.push(PreviewHeaderSegment::new(
                HEADER_SCORE_DETAIL,
                header_detail,
                compact_freeform_header_text(&detail, FREEFORM_COMPACT_WIDTH)
                    .and_then(|compact| (compact != detail).then_some(compact)),
            ));
        }

        if let Some(segment) = preview_line_coverage_header_segment(content.line_coverage) {
            segments.push(segment);
        }

        if let Some(note) = content.status_note.as_deref()
            && !note.is_empty()
        {
            let note = sanitize_terminal_text(note);
            segments.push(PreviewHeaderSegment::new(
                HEADER_SCORE_STATUS,
                note.clone(),
                compact_status_note(&note)
                    .or_else(|| compact_freeform_header_text(&note, FREEFORM_COMPACT_WIDTH)),
            ));
        }

        if let Some(title) = content
            .navigation_position
            .as_ref()
            .and_then(|position| position.title.as_deref())
            .filter(|title| !title.is_empty())
            .filter(|_| content.ebook_section_count.is_none())
        {
            let title = sanitize_terminal_text(title);
            segments.push(PreviewHeaderSegment::new(
                HEADER_SCORE_TITLE,
                title.clone(),
                compact_freeform_header_text(&title, FREEFORM_COMPACT_WIDTH),
            ));
        }

        let has_primary_parts = content
            .detail
            .as_deref()
            .is_some_and(|detail| !detail.is_empty())
            || content
                .status_note
                .as_deref()
                .is_some_and(|note| !note.is_empty())
            || content.line_coverage.is_some()
            || content.source_lines.is_some()
            || content.truncation_note.is_some();

        if content.line_coverage.is_none() {
            if let Some(source_lines) = content.source_lines {
                segments.push(PreviewHeaderSegment::new(
                    HEADER_SCORE_CONTEXT,
                    format!("{source_lines} lines"),
                    Some(format!("{source_lines}l")),
                ));
            } else if !has_primary_parts && content.kind != PreviewKind::Directory {
                let rendered_total = content.total_lines();
                if rendered_total > 0 {
                    let start = self.preview_state.scroll.saturating_add(1);
                    let end = (self.preview_state.scroll + visible_rows.max(1)).min(rendered_total);
                    let range = if rendered_total > visible_rows.max(1) {
                        format!("{start}-{end} / {rendered_total}")
                    } else {
                        format!("{rendered_total} lines")
                    };
                    segments.push(PreviewHeaderSegment::new(HEADER_SCORE_CONTEXT, range, None));
                }
            }
        } else if !has_primary_parts && content.kind != PreviewKind::Directory {
            segments.push(PreviewHeaderSegment::new(
                HEADER_SCORE_CONTEXT,
                format!("{} lines", content.total_lines()),
                None,
            ));
        }

        if let Some(image_detail) = self.static_image_preview_header_detail() {
            segments.push(PreviewHeaderSegment::new(
                HEADER_SCORE_CONTEXT,
                image_detail,
                None,
            ));
        }

        if content.line_coverage.is_none() {
            let wrapped_note =
                if content.truncation_note.is_none() && self.frame_state.preview_cols_visible > 0 {
                    content.wrapped_truncation_note(self.frame_state.preview_cols_visible)
                } else {
                    None
                };

            if let Some(note) = content
                .truncation_note
                .as_deref()
                .map(sanitize_terminal_text)
                .or_else(|| wrapped_note.map(|note| sanitize_terminal_text(&note)))
            {
                for part in note
                    .split("  •  ")
                    .filter(|part| !part.is_empty())
                    .map(str::to_string)
                {
                    segments.push(PreviewHeaderSegment::new(
                        HEADER_SCORE_AUXILIARY,
                        part.clone(),
                        compact_preview_header_note_part(&part)
                            .and_then(|compact| (compact != part).then_some(compact)),
                    ));
                }
            }
        }

        segments
    }

    fn pdf_preview_header_segment(&self) -> Option<PreviewHeaderSegment> {
        let full = self.pdf_preview_header_detail()?;
        let compact = full.strip_prefix("Page ").map(str::to_string);

        Some(PreviewHeaderSegment::new(
            HEADER_SCORE_NAVIGATION,
            full,
            compact,
        ))
    }
}

fn fit_preview_header_segments(
    segments: &[PreviewHeaderSegment],
    available_width: usize,
) -> Option<String> {
    if segments.is_empty() {
        return None;
    }
    if available_width == 0 {
        return Some(String::new());
    }

    let mut best_fit: Option<(u32, usize, String)> = None;
    let mut selected = Vec::with_capacity(segments.len());
    select_preview_header_variants(segments, 0, &mut selected, available_width, &mut best_fit);

    if let Some((_, _, label)) = best_fit {
        return Some(label);
    }

    fallback_preview_header_segment(segments)
        .map(|label| clamp_header_text(&label, available_width))
        .or_else(|| Some(String::new()))
}

fn select_preview_header_variants<'a>(
    segments: &'a [PreviewHeaderSegment],
    index: usize,
    selected: &mut Vec<&'a PreviewHeaderVariant>,
    available_width: usize,
    best_fit: &mut Option<(u32, usize, String)>,
) {
    if index == segments.len() {
        let visible = selected
            .iter()
            .filter_map(|variant| variant.text.as_deref())
            .collect::<Vec<_>>();
        if visible.is_empty() {
            return;
        }

        let label = visible.join(FITTED_HEADER_SEPARATOR);
        let width = UnicodeWidthStr::width(label.as_str());
        if width > available_width {
            return;
        }

        let score = selected.iter().map(|variant| variant.score).sum();
        match best_fit {
            Some((best_score, _, _)) if *best_score >= score => {}
            _ => *best_fit = Some((score, width, label)),
        }
        return;
    }

    for variant in &segments[index].variants {
        selected.push(variant);
        select_preview_header_variants(segments, index + 1, selected, available_width, best_fit);
        selected.pop();
    }
}

fn fallback_preview_header_segment(segments: &[PreviewHeaderSegment]) -> Option<String> {
    segments
        .iter()
        .filter_map(|segment| {
            let variant = segment
                .variants
                .iter()
                .find_map(|variant| variant.text.as_ref().map(|text| (variant.score, text)))?;
            Some((variant.0, variant.1.clone()))
        })
        .max_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then(left.1.len().cmp(&right.1.len()).reverse())
        })
        .map(|(_, label)| label)
}

fn compact_preview_header_label(label: &str) -> Option<String> {
    let compact = match label {
        "Comic ZIP archive" => "CBZ".to_string(),
        "Comic RAR archive" => "CBR".to_string(),
        "EPUB ebook" => "EPUB".to_string(),
        "PDF document" => "PDF".to_string(),
        "JSON with comments" => "JSONC".to_string(),
        "JSON5 file" => "JSON5".to_string(),
        "BitTorrent file" => "Torrent".to_string(),
        _ => strip_preview_header_suffix(label)?,
    };
    (compact != label).then_some(compact)
}

fn strip_preview_header_suffix(label: &str) -> Option<String> {
    const SUFFIXES: [&str; 13] = [
        " source file",
        " configuration file",
        " document",
        " ebook",
        " data file",
        " spreadsheet",
        " presentation",
        " stylesheet",
        " script",
        " archive",
        " image",
        " config",
        " file",
    ];

    SUFFIXES.iter().find_map(|suffix| {
        label
            .strip_suffix(suffix)
            .filter(|prefix| !prefix.is_empty())
            .map(str::to_string)
    })
}

fn preview_line_coverage_header_segment(
    coverage: Option<PreviewLineCoverage>,
) -> Option<PreviewHeaderSegment> {
    let coverage = coverage?;
    let full = format_preview_line_coverage(coverage, false);
    let compact =
        Some(format_preview_line_coverage(coverage, true)).filter(|compact| compact != &full);
    Some(PreviewHeaderSegment::new(
        HEADER_SCORE_LINE_COVERAGE,
        full,
        compact,
    ))
}

fn format_preview_line_coverage(coverage: PreviewLineCoverage, compact: bool) -> String {
    let shown_lines = format_header_count(coverage.shown_lines);
    if !coverage.partial {
        return format_line_label(coverage.total_lines.unwrap_or(coverage.shown_lines));
    }

    match coverage.total_lines {
        Some(total_lines) if coverage.shown_lines < total_lines => {
            let total_lines = format_header_count(total_lines);
            if compact {
                format!("{shown_lines} / {total_lines} shown")
            } else {
                format!("{shown_lines} / {total_lines} lines shown")
            }
        }
        Some(total_lines) => {
            let line_label = format_line_label(total_lines);
            if compact {
                format!("partial · {line_label}")
            } else {
                format!("partial file · {line_label}")
            }
        }
        None => {
            let line_label = format_line_label(coverage.shown_lines);
            if compact {
                format!("{shown_lines} shown")
            } else {
                format!("{line_label} shown")
            }
        }
    }
}

fn format_line_label(count: usize) -> String {
    let count = format_header_count(count);
    if count == "1" {
        "1 line".to_string()
    } else {
        format!("{count} lines")
    }
}

fn format_header_count(count: usize) -> String {
    let digits = count.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped
}

#[cfg(test)]
fn compact_preview_header_note(note: &str) -> Option<String> {
    let compact = note
        .split("  •  ")
        .map(|part| compact_preview_header_note_part(part).unwrap_or_else(|| part.to_string()))
        .collect::<Vec<_>>()
        .join(FITTED_HEADER_SEPARATOR);
    (compact != note).then_some(compact)
}

fn compact_preview_header_note_part(part: &str) -> Option<String> {
    if let Some(rest) = part.strip_prefix("truncated to ") {
        return Some(format!("{rest} cap"));
    }

    if let Some(value) = part
        .strip_prefix("showing first ")
        .and_then(|rest| rest.strip_suffix(" lines"))
    {
        return Some(format!("{value}-line cap"));
    }

    if let Some(value) = part
        .strip_prefix("showing first ")
        .and_then(|rest| rest.strip_suffix(" items"))
    {
        return Some(format!("{value}-item cap"));
    }

    if let Some(value) = part
        .strip_prefix("showing first ")
        .and_then(|rest| rest.strip_suffix(" wrapped"))
    {
        return Some(format!("{value} wrapped"));
    }

    if let Some(rest) = part.strip_prefix("showing first ")
        && let Some((shown, tail)) = rest.split_once(" of ")
        && let Some(total) = tail.strip_suffix(" entries")
    {
        return Some(format!("{shown}/{total} entries"));
    }

    if let Some(rest) = part.strip_prefix("showing first ")
        && let Some((shown, tail)) = rest.split_once(" of ")
        && let Some(total) = tail.strip_suffix(" files")
    {
        return Some(format!("{shown}/{total} files"));
    }

    None
}

fn compact_status_note(note: &str) -> Option<String> {
    let compact = match note {
        "Refreshing in background" => "Refreshing".to_string(),
        "Refresh unavailable" => "Refresh unavailable".to_string(),
        "Preview worker unavailable" => "Worker unavailable".to_string(),
        "Preparing cover preview" => "Preparing cover".to_string(),
        "Extracting comic page in background" => "Extracting page".to_string(),
        "Extracting ebook section in background" => "Extracting section".to_string(),
        _ => return None,
    };
    (compact != note).then_some(compact)
}

fn compact_freeform_header_text(text: &str, max_width: usize) -> Option<String> {
    let compact = clamp_header_text(text, max_width);
    (compact != text).then_some(compact)
}

fn clamp_header_text(text: &str, max_width: usize) -> String {
    let text = sanitize_terminal_text(text);
    if UnicodeWidthStr::width(text.as_str()) <= max_width {
        return text;
    }
    if max_width <= 1 {
        return "…".to_string();
    }

    let mut result = String::new();
    let mut width = 0usize;
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > max_width - 1 {
            break;
        }
        result.push(ch);
        width += ch_width;
    }
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_segment(weight: u32, full: &str, compact: Option<&str>) -> PreviewHeaderSegment {
        PreviewHeaderSegment::new(weight, full.to_string(), compact.map(str::to_string))
    }

    #[test]
    fn fitted_preview_header_prefers_compact_type_and_drops_auxiliary_notes() {
        let detail = header_segment(HEADER_SCORE_DETAIL, "Rust source file", Some("Rust"));
        let lines = header_segment(HEADER_SCORE_CONTEXT, "300 lines", Some("300l"));
        let truncated = header_segment(
            HEADER_SCORE_AUXILIARY,
            "truncated to 64 KiB",
            Some("64 KiB cap"),
        );

        let fitted = fit_preview_header_segments(&[detail, lines, truncated], 20);

        assert_eq!(fitted.as_deref(), Some("Rust • 300 lines"));
    }

    #[test]
    fn fitted_preview_header_keeps_navigation_before_optional_title() {
        let navigation = header_segment(HEADER_SCORE_NAVIGATION, "Section 2/14", Some("2/14"));
        let detail = header_segment(HEADER_SCORE_DETAIL, "EPUB ebook", Some("EPUB"));
        let title = header_segment(
            HEADER_SCORE_TITLE,
            "The Boy From The Wastes",
            Some("The Boy From The…"),
        );

        let fitted = fit_preview_header_segments(&[navigation, detail, title], 14);

        assert_eq!(fitted.as_deref(), Some("2/14 • EPUB"));
    }

    #[test]
    fn compact_preview_header_note_shortens_common_truncation_phrases() {
        assert_eq!(
            compact_preview_header_note("truncated to 64 KiB  •  showing first 240 lines")
                .as_deref(),
            Some("64 KiB cap • 240-line cap")
        );
    }

    #[test]
    fn compact_preview_header_label_shortens_comic_rar_archive() {
        assert_eq!(
            compact_preview_header_label("Comic RAR archive").as_deref(),
            Some("CBR")
        );
    }
}
