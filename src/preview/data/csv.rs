use super::*;
use std::collections::HashSet;

const MAX_PREVIEW_ROWS: usize = 50;
const MAX_COLS: usize = 20;
const MAX_COL_WIDTH: usize = 30;

pub(in crate::preview) fn build_csv_preview(
    text: &str,
    is_tsv: bool,
    type_detail: Option<&'static str>,
    bytes_truncated: bool,
) -> PreviewContent {
    // Strip UTF-8 BOM (\xEF\xBB\xBF) that Excel and some tools prepend.
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let sep = if is_tsv { '\t' } else { ',' };
    let mut records = parse_delimited(text, sep);

    let detail = type_detail.unwrap_or(if is_tsv { "TSV file" } else { "CSV file" });

    if records.is_empty() {
        return PreviewContent::new(PreviewKind::Data, vec![Line::from("Empty file")])
            .with_detail(detail);
    }

    // Detect whether we hit the internal row cap (independent of the 64 KiB read limit).
    let rows_truncated = records.len() > MAX_PREVIEW_ROWS;
    if rows_truncated {
        records.truncate(MAX_PREVIEW_ROWS);
    }

    let col_count = records
        .iter()
        .map(|r| r.len())
        .max()
        .unwrap_or(0)
        .min(MAX_COLS);

    if col_count == 0 {
        return PreviewContent::new(PreviewKind::Data, vec![Line::from("No columns")])
            .with_detail(detail);
    }

    // Normalize all records to exactly col_count fields.
    let records: Vec<Vec<String>> = records
        .into_iter()
        .map(|mut r| {
            r.truncate(col_count);
            r.resize(col_count, String::new());
            r
        })
        .collect();

    // Detect whether the first row is a header.
    let (headers, data_rows) = if records.len() >= 2
        && looks_like_header(&records[0], &records[1..])
    {
        let mut it = records.into_iter();
        let h = it.next().unwrap();
        (h, it.collect::<Vec<_>>())
    } else {
        let synthetic = (1..=col_count).map(|i| format!("col{i}")).collect();
        (synthetic, records)
    };

    let row_count = data_rows.len();

    // Column widths: max of header and data values, capped at MAX_COL_WIDTH.
    let mut col_widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in &data_rows {
        for (i, cell) in row.iter().enumerate() {
            if i < col_widths.len() {
                col_widths[i] = col_widths[i].max(cell.chars().count());
            }
        }
    }
    for w in &mut col_widths {
        *w = (*w).min(MAX_COL_WIDTH);
    }

    // A column is numeric if every non-empty data value parses as f64.
    let is_numeric: Vec<bool> = (0..col_count)
        .map(|i| {
            let has_any = data_rows.iter().any(|r| !r[i].is_empty());
            has_any
                && data_rows
                    .iter()
                    .all(|r| r[i].is_empty() || r[i].trim().parse::<f64>().is_ok())
        })
        .collect();

    let palette = theme::palette();
    let mut lines = Vec::new();

    lines.push(render_row(
        &headers,
        &col_widths,
        &is_numeric,
        true,
        palette,
    ));

    let sep_line: String = col_widths
        .iter()
        .map(|w| "─".repeat(*w))
        .collect::<Vec<_>>()
        .join("  ");
    lines.push(Line::from(Span::styled(
        sep_line,
        Style::default().fg(palette.muted),
    )));

    for row in &data_rows {
        lines.push(render_row(row, &col_widths, &is_numeric, false, palette));
    }

    lines.push(Line::from(""));
    let footer = match (bytes_truncated, rows_truncated) {
        // File was cut at the 64 KiB read limit — there may be more rows after the cut.
        (true, _) => format!("first {row_count} rows · {col_count} columns  (truncated at 64 KiB)"),
        // All bytes were read, but we capped display at MAX_PREVIEW_ROWS.
        (false, true) => format!("first {row_count} rows · {col_count} columns  (more rows in file)"),
        (false, false) => format!("{row_count} rows · {col_count} columns"),
    };
    lines.push(Line::from(Span::styled(
        footer,
        Style::default().fg(palette.muted),
    )));

    PreviewContent::new(PreviewKind::Data, lines).with_detail(detail)
}

// ── Parser ────────────────────────────────────────────────────────────────────

fn parse_delimited(text: &str, sep: char) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    let mut chars = text.chars().peekable();

    while chars.peek().is_some() {
        if records.len() > MAX_PREVIEW_ROWS {
            break;
        }
        let record = parse_record(&mut chars, sep);
        // Keep empty records that are followed by more content (mid-file blank lines),
        // but drop a final trailing empty record produced by a trailing newline.
        if record.iter().any(|f| !f.is_empty()) || chars.peek().is_some() {
            records.push(record);
        }
    }

    records
}

fn parse_record(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, sep: char) -> Vec<String> {
    let mut fields = Vec::new();
    loop {
        let (field, end_of_record) = parse_field(chars, sep);
        fields.push(field);
        if end_of_record || chars.peek().is_none() {
            break;
        }
    }
    fields
}

fn parse_field(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    sep: char,
) -> (String, bool) {
    match chars.peek().copied() {
        None => (String::new(), true),
        Some('"') => {
            chars.next();
            let mut field = String::new();
            loop {
                match chars.next() {
                    None => break,
                    Some('"') => {
                        if chars.peek() == Some(&'"') {
                            chars.next();
                            field.push('"');
                        } else {
                            break;
                        }
                    }
                    Some(c) => field.push(c),
                }
            }
            (field, advance_past_separator(chars, sep))
        }
        _ => {
            let mut field = String::new();
            loop {
                match chars.peek().copied() {
                    None => return (field, true),
                    Some(c) if c == sep => {
                        chars.next();
                        return (field, false);
                    }
                    Some('\n') => {
                        chars.next();
                        return (field, true);
                    }
                    Some('\r') => {
                        chars.next();
                        if chars.peek() == Some(&'\n') {
                            chars.next();
                        }
                        return (field, true);
                    }
                    Some(c) => {
                        chars.next();
                        field.push(c);
                    }
                }
            }
        }
    }
}

/// After closing a quoted field, skip past any garbage to the next separator
/// or record boundary. Returns true if a record boundary was reached.
fn advance_past_separator(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    sep: char,
) -> bool {
    loop {
        match chars.peek().copied() {
            None => return true,
            Some(c) if c == sep => {
                chars.next();
                return false;
            }
            Some('\n') => {
                chars.next();
                return true;
            }
            Some('\r') => {
                chars.next();
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                return true;
            }
            _ => {
                chars.next();
            }
        }
    }
}

// ── Header heuristic ──────────────────────────────────────────────────────────

/// Returns true when `first` looks like a header row:
/// all fields are non-empty, unique, non-numeric, and at least one data row
/// contains a numeric value (confirming that the data is more "typed" than
/// the header).
fn looks_like_header(first: &[String], data_rows: &[Vec<String>]) -> bool {
    if data_rows.is_empty() {
        return false;
    }
    if first.iter().any(|v| v.trim().is_empty()) {
        return false;
    }
    // Values must be unique (case-insensitive)
    let mut seen = HashSet::new();
    for v in first {
        if !seen.insert(v.to_lowercase()) {
            return false;
        }
    }
    // Header values must all be non-numeric
    if first.iter().any(|v| v.trim().parse::<f64>().is_ok()) {
        return false;
    }
    // At least one data row must contain a numeric value; otherwise the file
    // is ambiguously all-text and we prefer synthetic headers.
    data_rows
        .iter()
        .any(|row| row.iter().any(|v| !v.is_empty() && v.trim().parse::<f64>().is_ok()))
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn render_row(
    cells: &[String],
    widths: &[usize],
    is_numeric: &[bool],
    is_header: bool,
    palette: theme::Palette,
) -> Line<'static> {
    let color = if is_header {
        palette.accent
    } else {
        palette.text
    };
    let last = cells.len().saturating_sub(1);
    let mut spans = Vec::new();

    for (i, (cell, width)) in cells.iter().zip(widths.iter()).enumerate() {
        let display = truncate_to_width(cell, *width);
        let aligned = if is_numeric.get(i).copied().unwrap_or(false) {
            format!("{display:>width$}", width = width)
        } else {
            format!("{display:<width$}", width = width)
        };
        spans.push(Span::styled(aligned, Style::default().fg(color)));
        if i < last {
            spans.push(Span::styled("  ".to_string(), Style::default().fg(palette.muted)));
        }
    }

    Line::from(spans)
}

fn truncate_to_width(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{truncated}…")
}
