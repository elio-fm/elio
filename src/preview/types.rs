use crate::fs as browser_support;
use ratatui::text::Line;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use super::layout::{
    WrappedLayoutCache, WrappedPreviewLines, sanitize_preview_lines, wrap_preview_lines,
};

pub(super) const PREVIEW_LIMIT_BYTES: usize = 64 * 1024;
pub(super) const PREVIEW_RENDER_LINE_LIMIT: usize = 800;
pub(crate) const MARKDOWN_CONTENT_WIDTH: usize = 100;
pub(crate) const MIN_DYNAMIC_CODE_PREVIEW_LINE_LIMIT: usize = 80;
pub(super) const PREVIEW_WRAPPED_LINE_LIMIT: usize = PREVIEW_RENDER_LINE_LIMIT;
const WRAPPED_LAYOUT_CACHE_LIMIT: usize = 4;

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) enum PreviewRequestOptions {
    #[default]
    Default,
    EpubSection(usize),
    ComicPage(usize),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PreviewWorkClass {
    Light,
    Heavy,
}

impl PreviewRequestOptions {
    pub(crate) fn epub_section_index(&self) -> Option<usize> {
        match self {
            Self::Default => None,
            Self::EpubSection(index) => Some(*index),
            Self::ComicPage(_) => None,
        }
    }

    pub(crate) fn comic_page_index(&self) -> Option<usize> {
        match self {
            Self::ComicPage(index) => Some(*index),
            Self::Default | Self::EpubSection(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewKind {
    Audio,
    Archive,
    Comic,
    Data,
    Directory,
    Document,
    Image,
    Video,
    Markdown,
    Code,
    Text,
    Binary,
    Unavailable,
}

impl PreviewKind {
    pub(crate) fn section_label(self) -> &'static str {
        match self {
            Self::Audio => "Audio",
            Self::Archive => "Archive",
            Self::Comic => "Comic",
            Self::Data => "Data",
            Self::Directory => "Contents",
            Self::Document => "Document",
            Self::Image => "Image",
            Self::Video => "Video",
            Self::Markdown => "Markdown",
            Self::Code => "Code",
            Self::Text => "Text",
            Self::Binary | Self::Unavailable => "Preview",
        }
    }

    pub(crate) fn wraps_in_preview(self) -> bool {
        matches!(
            self,
            Self::Audio
                | Self::Comic
                | Self::Document
                | Self::Image
                | Self::Video
                | Self::Text
                | Self::Binary
                | Self::Unavailable
        )
    }

    pub(crate) fn allows_horizontal_scroll(self) -> bool {
        matches!(
            self,
            Self::Code | Self::Data | Self::Markdown | Self::Archive | Self::Directory
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PreviewVisualKind {
    Cover,
    PageImage,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PreviewVisualLayout {
    Inline,
    LargeInline,
    FullHeight,
}

pub(crate) fn default_code_preview_line_limit() -> usize {
    PREVIEW_RENDER_LINE_LIMIT
}

pub(crate) fn clamp_code_preview_line_limit(line_limit: usize) -> usize {
    line_limit.clamp(1, PREVIEW_RENDER_LINE_LIMIT)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreviewVisual {
    pub kind: PreviewVisualKind,
    pub layout: PreviewVisualLayout,
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreviewNavigationPosition {
    pub label: &'static str,
    pub index: usize,
    pub count: usize,
    pub title: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreviewLineCoverage {
    pub shown_lines: usize,
    pub total_lines: Option<usize>,
    pub total_lines_pending: bool,
    pub partial: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct PreviewContent {
    pub kind: PreviewKind,
    pub detail: Option<String>,
    pub status_note: Option<String>,
    pub preview_visual: Option<PreviewVisual>,
    pub navigation_position: Option<PreviewNavigationPosition>,
    pub ebook_section_index: Option<usize>,
    pub ebook_section_count: Option<usize>,
    pub ebook_section_title: Option<String>,
    pub truncated: bool,
    pub truncation_note: Option<String>,
    pub source_lines: Option<usize>,
    pub line_coverage: Option<PreviewLineCoverage>,
    pub item_count: Option<usize>,
    pub folder_count: Option<usize>,
    pub file_count: Option<usize>,
    pub lines: Arc<[Line<'static>]>,
    /// When `Some(n)`, only the first `n` source lines were rendered.
    /// The full file has more lines available and an extension job should be
    /// submitted to replace this partial render with a complete one.
    /// `None` means the render covers all available lines.
    pub(crate) incremental_render_limit: Option<usize>,
    max_line_width: usize,
    wrapped_layout_cache: Arc<Mutex<WrappedLayoutCache>>,
}

impl PreviewContent {
    pub(crate) fn new(kind: PreviewKind, lines: Vec<Line<'static>>) -> Self {
        let lines = sanitize_preview_lines(lines);
        let max_line_width = lines.iter().map(Line::width).max().unwrap_or(0);
        Self {
            kind,
            detail: None,
            status_note: None,
            preview_visual: None,
            navigation_position: None,
            ebook_section_index: None,
            ebook_section_count: None,
            ebook_section_title: None,
            truncated: false,
            truncation_note: None,
            source_lines: None,
            line_coverage: None,
            item_count: None,
            folder_count: None,
            file_count: None,
            lines,
            incremental_render_limit: None,
            max_line_width,
            wrapped_layout_cache: Arc::new(Mutex::new(WrappedLayoutCache::default())),
        }
    }

    /// Returns `true` when this preview only covers a partial set of source
    /// lines and a full-file extension render is expected to follow.
    pub(crate) fn is_incrementally_partial(&self) -> bool {
        self.incremental_render_limit.is_some()
    }

    pub(crate) fn placeholder(label: &str) -> Self {
        Self::new(
            PreviewKind::Unavailable,
            vec![Line::from(label.to_string())],
        )
    }

    pub(crate) fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub(crate) fn with_status_note(mut self, note: impl Into<String>) -> Self {
        self.status_note = Some(note.into());
        self
    }

    pub(crate) fn with_preview_visual(mut self, visual: PreviewVisual) -> Self {
        self.preview_visual = Some(visual);
        self
    }

    pub(crate) fn with_navigation_position(
        mut self,
        label: &'static str,
        index: usize,
        count: usize,
        title: Option<String>,
    ) -> Self {
        self.navigation_position = Some(PreviewNavigationPosition {
            label,
            index,
            count: count.max(1),
            title: title.filter(|title| !title.is_empty()),
        });
        self
    }

    pub(crate) fn with_ebook_section(
        mut self,
        index: usize,
        count: usize,
        title: Option<String>,
    ) -> Self {
        let title = title.filter(|title| !title.is_empty());
        self.navigation_position = Some(PreviewNavigationPosition {
            label: "Section",
            index,
            count: count.max(1),
            title: title.clone(),
        });
        self.ebook_section_index = Some(index);
        self.ebook_section_count = Some(count.max(1));
        self.ebook_section_title = title;
        self
    }

    pub(crate) fn with_source_lines(mut self, source_lines: usize) -> Self {
        self.source_lines = Some(source_lines.max(1));
        self
    }

    pub(crate) fn with_line_coverage(
        mut self,
        shown_lines: usize,
        total_lines: Option<usize>,
        partial: bool,
    ) -> Self {
        self.line_coverage = Some(PreviewLineCoverage {
            shown_lines: shown_lines.max(1),
            total_lines: total_lines.map(|count| count.max(shown_lines.max(1))),
            total_lines_pending: false,
            partial,
        });
        self
    }

    pub(crate) fn with_truncation(mut self, note: impl Into<String>) -> Self {
        self.truncated = true;
        self.truncation_note = Some(note.into());
        self
    }

    pub(crate) fn needs_total_line_count(&self) -> bool {
        self.line_coverage
            .as_ref()
            .is_some_and(|coverage| coverage.partial && coverage.total_lines.is_none())
    }

    pub(crate) fn set_total_line_count_pending(&mut self, pending: bool) {
        if let Some(coverage) = &mut self.line_coverage
            && coverage.partial
            && coverage.total_lines.is_none()
        {
            coverage.total_lines_pending = pending;
        }
    }

    pub(crate) fn apply_total_line_count(&mut self, total_lines: usize) {
        let total_lines = total_lines.max(1);
        if let Some(coverage) = &mut self.line_coverage {
            coverage.total_lines = Some(total_lines.max(coverage.shown_lines));
            coverage.total_lines_pending = false;
        }
    }

    pub(crate) fn with_directory_counts(
        mut self,
        item_count: usize,
        folder_count: usize,
        file_count: usize,
    ) -> Self {
        self.item_count = Some(item_count);
        self.folder_count = Some(folder_count);
        self.file_count = Some(file_count);
        self
    }

    pub(crate) fn section_label(&self) -> &'static str {
        self.kind.section_label()
    }

    pub(crate) fn total_lines(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn lines(&self) -> Vec<Line<'static>> {
        self.lines.iter().cloned().collect()
    }

    pub(crate) fn wrapped_lines(&self, width: usize) -> Arc<[Line<'static>]> {
        Arc::clone(&self.wrapped_layout(width).lines)
    }

    pub(crate) fn wrapped_truncation_note(&self, width: usize) -> Option<String> {
        self.kind
            .wraps_in_preview()
            .then(|| self.wrapped_layout(width).truncated)
            .filter(|truncated| *truncated)
            .map(|_| format!("first {PREVIEW_WRAPPED_LINE_LIMIT} wrapped"))
    }

    pub(crate) fn visual_line_count(&self, width: usize) -> usize {
        if !self.kind.wraps_in_preview() {
            return self.total_lines();
        }
        self.wrapped_layout(width).lines.len().max(1)
    }

    fn wrapped_layout(&self, width: usize) -> Arc<WrappedPreviewLines> {
        if !self.kind.wraps_in_preview() {
            return Arc::new(WrappedPreviewLines {
                lines: Arc::clone(&self.lines),
                max_line_width: self.max_line_width,
                truncated: false,
            });
        }

        // Cap markdown at MARKDOWN_CONTENT_WIDTH so prose wraps at a GitHub-like
        // column limit rather than expanding to fill the full terminal width.
        let width = if self.kind == PreviewKind::Markdown {
            MARKDOWN_CONTENT_WIDTH.min(width)
        } else {
            width
        }
        .max(1);
        if let Some(layout) = self.cached_wrapped_layout(width) {
            return layout;
        }

        let wrapped = Arc::new(wrap_preview_lines(&self.lines, width));
        let mut cache = self
            .wrapped_layout_cache
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(existing) = cache.lines_by_width.get(&width) {
            return Arc::clone(existing);
        }
        cache
            .width_order
            .retain(|cached_width| *cached_width != width);
        cache.width_order.push_back(width);
        cache.lines_by_width.insert(width, Arc::clone(&wrapped));
        while cache.width_order.len() > WRAPPED_LAYOUT_CACHE_LIMIT {
            if let Some(stale_width) = cache.width_order.pop_front() {
                cache.lines_by_width.remove(&stale_width);
            }
        }
        wrapped
    }

    pub(crate) fn wrapped_max_line_width(&self, width: usize) -> usize {
        self.wrapped_layout(width).max_line_width
    }

    #[cfg(test)]
    pub(crate) fn header_detail(&self, offset: usize, visible_rows: usize) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(detail) = &self.detail
            && !detail.is_empty()
        {
            parts.push(browser_support::sanitize_terminal_text(detail));
        }

        if let Some(note) = &self.status_note
            && !note.is_empty()
        {
            parts.push(browser_support::sanitize_terminal_text(note));
        }

        if let Some(source_lines) = self.source_lines {
            parts.push(format!("{source_lines} lines"));
        }

        if let Some(note) = &self.truncation_note {
            parts.push(browser_support::sanitize_terminal_text(note));
        }

        if !parts.is_empty() {
            return Some(parts.join("  •  "));
        }

        if self.kind == PreviewKind::Directory {
            return None;
        }

        let rendered_total = self.total_lines();
        if rendered_total == 0 {
            return self.detail.clone();
        }

        let start = offset.saturating_add(1);
        let end = (offset + visible_rows.max(1)).min(rendered_total);
        let range = if rendered_total > visible_rows.max(1) {
            format!("{start}-{end} / {rendered_total}")
        } else {
            format!("{rendered_total} lines")
        };

        match &self.detail {
            Some(detail) if !detail.is_empty() => Some(format!("{detail}  •  {range}")),
            _ => Some(range),
        }
    }

    #[cfg(test)]
    pub(crate) fn navigation_header_detail(&self) -> Option<String> {
        let position = self.navigation_position.as_ref()?;
        let label = format!(
            "{} {}/{}",
            position.label,
            position.index + 1,
            position.count
        );
        if self.ebook_section_count.is_some() {
            return Some(label);
        }
        match position.title.as_deref() {
            Some(title) if !title.is_empty() => Some(format!(
                "{label}  •  {}",
                browser_support::sanitize_terminal_text(title)
            )),
            _ => Some(label),
        }
    }

    fn cached_wrapped_layout(&self, width: usize) -> Option<Arc<WrappedPreviewLines>> {
        self.wrapped_layout_cache
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .lines_by_width
            .get(&width)
            .cloned()
    }
}

pub(super) fn status_preview(
    kind: PreviewKind,
    detail: impl Into<String>,
    lines: impl IntoIterator<Item = Line<'static>>,
) -> PreviewContent {
    PreviewContent::new(kind, lines.into_iter().collect()).with_detail(detail)
}

pub(super) fn unavailable_directory_preview(error: &std::io::Error) -> PreviewContent {
    let detail = browser_support::describe_io_error(error);
    let message = match error.kind() {
        std::io::ErrorKind::PermissionDenied => "You do not have permission to open this folder",
        std::io::ErrorKind::NotFound => "This folder is no longer available",
        std::io::ErrorKind::Unsupported => "This location is not supported",
        _ => "Folder preview unavailable",
    };
    unavailable_preview(detail, message)
}

fn unavailable_preview(detail: &str, message: &str) -> PreviewContent {
    status_preview(
        PreviewKind::Unavailable,
        detail,
        [
            Line::from("Preview unavailable"),
            Line::from(message.to_string()),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{
        style::{Color, Style},
        text::Span,
    };

    #[test]
    fn wrapped_preview_lines_cache_by_width() {
        let preview = PreviewContent::new(
            PreviewKind::Text,
            vec![Line::from("alpha beta gamma delta epsilon")],
        );

        let first = preview.wrapped_lines(8);
        let second = preview.wrapped_lines(8);

        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(preview.visual_line_count(8), first.len());
    }

    #[test]
    fn wrapped_preview_lines_preserve_text_and_styles() {
        let preview = PreviewContent::new(
            PreviewKind::Text,
            vec![Line::from(vec![
                Span::styled("abcdef", Style::default().fg(Color::Red)),
                Span::styled("ghij", Style::default().fg(Color::Blue)),
            ])],
        );

        let wrapped = preview.wrapped_lines(6);

        assert_eq!(wrapped.len(), 2);
        assert_eq!(wrapped[0].to_string(), "abcdef");
        assert_eq!(wrapped[1].to_string(), "ghij");
        assert_eq!(wrapped[0].spans[0].style.fg, Some(Color::Red));
        assert_eq!(wrapped[1].spans[0].style.fg, Some(Color::Blue));
    }

    #[test]
    fn wrapped_preview_lines_cap_visual_depth() {
        let preview = PreviewContent::new(PreviewKind::Text, vec![Line::from("a ".repeat(2_000))]);

        let wrapped = preview.wrapped_lines(4);
        let expected = format!("first {PREVIEW_WRAPPED_LINE_LIMIT} wrapped");

        assert_eq!(wrapped.len(), PREVIEW_WRAPPED_LINE_LIMIT);
        assert_eq!(
            preview.wrapped_truncation_note(4).as_deref(),
            Some(expected.as_str())
        );
    }

    #[test]
    fn preview_line_coverage_tracks_pending_and_total_counts() {
        let mut preview = PreviewContent::new(PreviewKind::Text, vec![Line::from("alpha")])
            .with_line_coverage(5, None, true);

        assert!(preview.needs_total_line_count());
        assert_eq!(
            preview.line_coverage,
            Some(PreviewLineCoverage {
                shown_lines: 5,
                total_lines: None,
                total_lines_pending: false,
                partial: true,
            })
        );

        preview.set_total_line_count_pending(true);
        assert_eq!(
            preview.line_coverage,
            Some(PreviewLineCoverage {
                shown_lines: 5,
                total_lines: None,
                total_lines_pending: true,
                partial: true,
            })
        );

        preview.apply_total_line_count(3);
        assert_eq!(
            preview.line_coverage,
            Some(PreviewLineCoverage {
                shown_lines: 5,
                total_lines: Some(5),
                total_lines_pending: false,
                partial: true,
            })
        );
        assert!(!preview.needs_total_line_count());
    }
}
