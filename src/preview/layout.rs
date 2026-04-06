use crate::fs as browser_support;
use ratatui::{
    layout::Alignment,
    style::Style,
    text::{Line, Span, StyledGrapheme},
};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use unicode_width::UnicodeWidthStr;

use super::appearance;
use super::types::PREVIEW_WRAPPED_LINE_LIMIT;

const NBSP: &str = "\u{00a0}";
const ZWSP: &str = "\u{200b}";

// ---------------------------------------------------------------------------
// Wrapped layout cache
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub(super) struct WrappedLayoutCache {
    pub(super) lines_by_width: HashMap<usize, Arc<WrappedPreviewLines>>,
    pub(super) width_order: VecDeque<usize>,
}

#[derive(Debug)]
pub(super) struct WrappedPreviewLines {
    pub(super) lines: Arc<[Line<'static>]>,
    pub(super) max_line_width: usize,
    pub(super) truncated: bool,
}

// ---------------------------------------------------------------------------
// Sanitization
// ---------------------------------------------------------------------------

pub(super) fn sanitize_preview_lines(lines: Vec<Line<'static>>) -> Arc<[Line<'static>]> {
    lines
        .into_iter()
        .map(sanitize_preview_line)
        .collect::<Vec<_>>()
        .into()
}

fn sanitize_preview_line(mut line: Line<'static>) -> Line<'static> {
    for span in &mut line.spans {
        let sanitized = browser_support::sanitize_terminal_text(span.content.as_ref());
        span.content = sanitized.into();
    }
    line
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

pub(crate) fn line_number_span(number: usize, width: usize) -> Span<'static> {
    let preview = appearance::code_palette();
    Span::styled(
        format!("{number:>width$} ", width = width),
        Style::default().fg(preview.line_number),
    )
}

pub(crate) fn line_number_width(lines: usize) -> usize {
    lines.max(1).to_string().len().max(3)
}

pub(crate) fn expand_tabs(text: &str) -> String {
    browser_support::sanitize_terminal_text(text)
}

// ---------------------------------------------------------------------------
// Word-wrap algorithm
// ---------------------------------------------------------------------------

pub(super) fn wrap_preview_lines(lines: &[Line<'static>], width: usize) -> WrappedPreviewLines {
    let width = width.max(1);
    let mut wrapped = Vec::new();
    let mut truncated = false;
    for (index, line) in lines.iter().enumerate() {
        if !wrap_preview_line(line, width, &mut wrapped, PREVIEW_WRAPPED_LINE_LIMIT) {
            truncated = true;
            break;
        }
        if wrapped.len() >= PREVIEW_WRAPPED_LINE_LIMIT && index + 1 < lines.len() {
            truncated = true;
            break;
        }
    }
    if wrapped.is_empty() {
        wrapped.push(Line::default());
    }
    let max_line_width = wrapped.iter().map(Line::width).max().unwrap_or(0);
    WrappedPreviewLines {
        lines: Arc::from(wrapped),
        max_line_width,
        truncated,
    }
}

fn wrap_preview_line<'a>(
    line: &'a Line<'static>,
    max_width: usize,
    wrapped: &mut Vec<Line<'static>>,
    line_limit: usize,
) -> bool {
    let mut pending_line = Vec::<StyledGrapheme<'a>>::new();
    let mut pending_word = Vec::<StyledGrapheme<'a>>::new();
    let mut pending_whitespace = VecDeque::<StyledGrapheme<'a>>::new();
    let mut line_width = 0usize;
    let mut word_width = 0usize;
    let mut whitespace_width = 0usize;
    let mut non_whitespace_previous = false;
    let alignment = line.alignment;
    let trim = false;

    for grapheme in line.styled_graphemes(Style::default()) {
        let is_whitespace = preview_grapheme_is_whitespace(grapheme.symbol);
        let symbol_width = grapheme.symbol.width();
        if symbol_width > max_width {
            continue;
        }

        let word_found = non_whitespace_previous && is_whitespace;
        let trimmed_overflow =
            pending_line.is_empty() && trim && word_width + symbol_width > max_width;
        let whitespace_overflow =
            pending_line.is_empty() && trim && whitespace_width + symbol_width > max_width;
        let untrimmed_overflow = pending_line.is_empty()
            && !trim
            && word_width + whitespace_width + symbol_width > max_width;

        if word_found || trimmed_overflow || whitespace_overflow || untrimmed_overflow {
            if !pending_line.is_empty() || !trim {
                pending_line.extend(pending_whitespace.drain(..));
                line_width += whitespace_width;
            }

            pending_line.append(&mut pending_word);
            line_width += word_width;

            whitespace_width = 0;
            word_width = 0;
        }

        let line_full = line_width >= max_width;
        let pending_word_overflow =
            symbol_width > 0 && line_width + whitespace_width + word_width >= max_width;

        if line_full || pending_word_overflow {
            let mut remaining_width = max_width.saturating_sub(line_width);
            if !push_wrapped_preview_line(wrapped, &pending_line, alignment, line_limit) {
                return false;
            }
            pending_line.clear();
            line_width = 0;

            while let Some(grapheme) = pending_whitespace.front() {
                let width = grapheme.symbol.width();
                if width > remaining_width {
                    break;
                }

                whitespace_width = whitespace_width.saturating_sub(width);
                remaining_width = remaining_width.saturating_sub(width);
                pending_whitespace.pop_front();
            }

            if is_whitespace && pending_whitespace.is_empty() {
                continue;
            }
        }

        if is_whitespace {
            whitespace_width += symbol_width;
            pending_whitespace.push_back(grapheme);
        } else {
            word_width += symbol_width;
            pending_word.push(grapheme);
        }

        non_whitespace_previous = !is_whitespace;
    }

    if pending_line.is_empty()
        && pending_word.is_empty()
        && !pending_whitespace.is_empty()
        && !push_wrapped_line(wrapped, empty_wrapped_preview_line(alignment), line_limit)
    {
        return false;
    }
    if !pending_line.is_empty() || !trim {
        pending_line.extend(pending_whitespace.drain(..));
    }
    pending_line.append(&mut pending_word);
    if pending_line.is_empty() {
        push_wrapped_line(wrapped, empty_wrapped_preview_line(alignment), line_limit)
    } else {
        push_wrapped_preview_line(wrapped, &pending_line, alignment, line_limit)
    }
}

fn push_wrapped_line(
    wrapped: &mut Vec<Line<'static>>,
    line: Line<'static>,
    line_limit: usize,
) -> bool {
    if wrapped.len() >= line_limit {
        return false;
    }
    wrapped.push(line);
    true
}

fn preview_grapheme_is_whitespace(symbol: &str) -> bool {
    symbol == ZWSP || (symbol != NBSP && symbol.chars().all(char::is_whitespace))
}

fn push_wrapped_preview_line(
    wrapped: &mut Vec<Line<'static>>,
    graphemes: &[StyledGrapheme<'_>],
    alignment: Option<Alignment>,
    line_limit: usize,
) -> bool {
    let line = Line {
        alignment,
        ..line_from_graphemes(graphemes)
    };
    push_wrapped_line(wrapped, line, line_limit)
}

fn empty_wrapped_preview_line(alignment: Option<Alignment>) -> Line<'static> {
    Line {
        alignment,
        ..Line::default()
    }
}

fn line_from_graphemes(graphemes: &[StyledGrapheme<'_>]) -> Line<'static> {
    if graphemes.is_empty() {
        return Line::default();
    }

    let mut spans = Vec::<Span<'static>>::new();
    let mut current_style = graphemes[0].style;
    let mut current_content = String::new();

    for grapheme in graphemes {
        if grapheme.style == current_style {
            current_content.push_str(grapheme.symbol);
            continue;
        }

        spans.push(Span::styled(
            std::mem::take(&mut current_content),
            current_style,
        ));
        current_style = grapheme.style;
        current_content.push_str(grapheme.symbol);
    }

    if !current_content.is_empty() {
        spans.push(Span::styled(current_content, current_style));
    }

    Line::from(spans)
}
