use super::*;
use crate::preview::{PreviewKind, PreviewLineCoverage};
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
pub(super) struct PreviewHeaderSegment {
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
    pub(super) fn preview_header_segments(&self, visible_rows: usize) -> Vec<PreviewHeaderSegment> {
        let mut segments = Vec::new();
        let content = &self.preview.state.content;
        let directory_stats_detail = self.preview_directory_stats_header_detail();
        let directory_stats_note = self.preview_directory_stats_status_note();

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

        if let Some((full, compact)) = directory_stats_detail.clone() {
            segments.push(PreviewHeaderSegment::new(
                HEADER_SCORE_DETAIL,
                full,
                compact,
            ));
        } else if let Some(detail) = content.detail.as_deref()
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

        if let Some((full, compact)) = directory_stats_note.clone() {
            segments.push(PreviewHeaderSegment::new(
                HEADER_SCORE_STATUS,
                full,
                compact,
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
            || directory_stats_detail.is_some()
            || content
                .status_note
                .as_deref()
                .is_some_and(|note| !note.is_empty())
            || directory_stats_note.is_some()
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
                    let start = self.preview.state.scroll.saturating_add(1);
                    let end = (self.preview.state.scroll + visible_rows.max(1)).min(rendered_total);
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
            let wrapped_note = if content.truncation_note.is_none()
                && self.input.frame_state.preview_cols_visible > 0
            {
                content.wrapped_truncation_note(self.input.frame_state.preview_cols_visible)
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

    fn preview_directory_stats_header_detail(&self) -> Option<(String, Option<String>)> {
        if self.preview.state.content.kind != PreviewKind::Directory
            || self.preview.state.load_state.is_some()
        {
            return None;
        }
        match self.preview.state.directory_stats.as_ref()? {
            PreviewDirectoryStatsState::Loading { .. } => None,
            PreviewDirectoryStatsState::Complete { stats, .. } => {
                let size = format_size(stats.total_size_bytes);
                let full = format!("{} • {size}", format_total_item_label(stats.item_count));
                let compact = Some(format!("{} • {size}", format_item_count(stats.item_count)))
                    .filter(|compact| compact != &full);
                Some((full, compact))
            }
            PreviewDirectoryStatsState::Incomplete { partial, .. } => {
                if partial.item_count == 0 && partial.total_size_bytes == 0 {
                    return None;
                }
                let size = format_size(partial.total_size_bytes);
                Some((
                    format!(
                        "At least {} • at least {size}",
                        format_item_count(partial.item_count)
                    ),
                    Some(format!(
                        "{}+ • {size}+",
                        format_item_count(partial.item_count)
                    )),
                ))
            }
        }
    }

    fn preview_directory_stats_status_note(&self) -> Option<(String, Option<String>)> {
        if self.preview.state.content.kind != PreviewKind::Directory
            || self.preview.state.load_state.is_some()
        {
            return None;
        }
        match self.preview.state.directory_stats.as_ref()? {
            PreviewDirectoryStatsState::Loading { .. } => None,
            PreviewDirectoryStatsState::Complete { .. } => None,
            PreviewDirectoryStatsState::Incomplete { error, .. } => {
                Some((error.clone(), compact_status_note(error)))
            }
        }
    }
}

fn format_total_item_label(count: usize) -> String {
    let count = format_header_count(count);
    if count == "1" {
        "1 total item".to_string()
    } else {
        format!("{count} total items")
    }
}

pub(super) fn fit_preview_header_segments(
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
        "Extracting comic page in background" => "Extracting page".to_string(),
        "Extracting ebook section in background" => "Extracting section".to_string(),
        "Some entries unreadable" => "Incomplete".to_string(),
        "Folder changed while scanning" => "Incomplete".to_string(),
        "Folder totals incomplete" => "Incomplete".to_string(),
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
        let line_limit = crate::preview::default_code_preview_line_limit();
        let note = format!("truncated to 64 KiB  •  showing first {line_limit} lines");
        let expected = format!("64 KiB cap • {line_limit}-line cap");
        assert_eq!(
            compact_preview_header_note(&note).as_deref(),
            Some(expected.as_str())
        );
    }

    #[test]
    fn compact_preview_header_label_shortens_comic_rar_archive() {
        assert_eq!(
            compact_preview_header_label("Comic RAR archive").as_deref(),
            Some("CBR")
        );
    }

    #[test]
    fn fitted_preview_header_clamps_fallback_segment_when_nothing_fits() {
        let detail = header_segment(HEADER_SCORE_DETAIL, "Rust source file", Some("Rust"));
        let line_limit = crate::preview::default_code_preview_line_limit();
        let full = format!("{line_limit} lines shown");
        let compact = format!("{line_limit} shown");
        let lines = header_segment(HEADER_SCORE_CONTEXT, full.as_str(), Some(compact.as_str()));

        let fitted = fit_preview_header_segments(&[detail, lines], 3);

        assert_eq!(fitted.as_deref(), Some("Ru…"));
    }
}
