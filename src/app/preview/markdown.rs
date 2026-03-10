use super::*;
use crate::appearance;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

pub(super) fn render_markdown_preview(text: &str) -> Vec<Line<'static>> {
    let palette = appearance::palette();
    let mut rendered = Vec::new();
    let mut fence_lang = None::<String>;
    let mut fence_lines = Vec::new();

    for raw_line in text.lines() {
        if rendered.len() >= PREVIEW_RENDER_LINE_LIMIT {
            break;
        }

        if let Some(lang) = fence_lang.as_ref() {
            if is_fence_delimiter(raw_line) {
                rendered.extend(render_markdown_fence(lang, &fence_lines));
                fence_lang = None;
                fence_lines.clear();
                continue;
            }
            fence_lines.push(raw_line.to_string());
            continue;
        }

        if let Some(lang) = parse_fence_start(raw_line) {
            fence_lang = Some(lang);
            continue;
        }

        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            rendered.push(Line::from(String::new()));
            continue;
        }

        if let Some((level, title)) = parse_heading(raw_line) {
            rendered.push(render_heading_line(level, title, palette));
            continue;
        }

        if is_thematic_break(trimmed) {
            rendered.push(Line::from(Span::styled(
                "────────────────",
                Style::default().fg(palette.border),
            )));
            continue;
        }

        if let Some(quoted) = trimmed.strip_prefix('>') {
            rendered.push(render_quote_line(quoted.trim_start(), palette));
            continue;
        }

        if let Some((checked, body, indent)) = parse_task_item(raw_line) {
            rendered.push(render_list_item(
                if checked { "󰄬" } else { "󰄱" },
                body,
                indent,
                palette,
            ));
            continue;
        }

        if let Some((body, indent)) = parse_unordered_item(raw_line) {
            rendered.push(render_list_item("•", body, indent, palette));
            continue;
        }

        if let Some((number, body, indent)) = parse_ordered_item(raw_line) {
            rendered.push(render_list_item(
                &format!("{number}."),
                body,
                indent,
                palette,
            ));
            continue;
        }

        rendered.push(Line::from(parse_inline_markdown(
            raw_line.trim_end(),
            palette,
        )));
    }

    if fence_lang.is_some() && rendered.len() < PREVIEW_RENDER_LINE_LIMIT {
        rendered.extend(render_markdown_fence(
            fence_lang.as_deref().unwrap_or("text"),
            &fence_lines,
        ));
    }

    rendered.truncate(PREVIEW_RENDER_LINE_LIMIT);
    if rendered.is_empty() {
        rendered.push(Line::from("File is empty"));
    }
    rendered
}

fn render_markdown_fence(language: &str, lines: &[String]) -> Vec<Line<'static>> {
    let palette = appearance::palette();
    let mut rendered = vec![Line::from(vec![
        Span::styled(
            "󰆍 ",
            Style::default()
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            language.to_string(),
            Style::default()
                .fg(palette.muted)
                .add_modifier(Modifier::ITALIC),
        ),
    ])];
    rendered.extend(render_plain_fence_body(lines));
    rendered
}

fn render_plain_fence_body(lines: &[String]) -> Vec<Line<'static>> {
    let palette = appearance::palette();
    let mut rendered = Vec::new();

    for line in lines
        .iter()
        .take(PREVIEW_RENDER_LINE_LIMIT.saturating_sub(1))
    {
        rendered.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(palette.border)),
            Span::styled(super::expand_tabs(line), Style::default().fg(palette.text)),
        ]));
    }

    if rendered.is_empty() {
        rendered.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(palette.border)),
            Span::styled("Code block is empty", Style::default().fg(palette.muted)),
        ]));
    }

    rendered
}

fn render_heading_line(level: usize, title: &str, palette: appearance::Palette) -> Line<'static> {
    let color = match level {
        1 => palette.accent_text,
        2 => palette.accent,
        3 => palette.text,
        _ => palette.muted,
    };
    Line::from(parse_inline_markdown_with_style(
        title.trim(),
        palette,
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    ))
}

fn render_quote_line(text: &str, palette: appearance::Palette) -> Line<'static> {
    let mut spans = vec![Span::styled("▎ ", Style::default().fg(palette.accent))];
    spans.extend(parse_inline_markdown(text, palette));
    Line::from(spans)
}

fn render_list_item(
    marker: &str,
    body: &str,
    indent: usize,
    palette: appearance::Palette,
) -> Line<'static> {
    let mut spans = vec![
        Span::raw(" ".repeat(indent * 2)),
        Span::styled(
            format!("{marker} "),
            Style::default()
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    spans.extend(parse_inline_markdown(body.trim(), palette));
    Line::from(spans)
}

fn parse_inline_markdown(input: &str, palette: appearance::Palette) -> Vec<Span<'static>> {
    parse_inline_markdown_with_style(input, palette, Style::default().fg(palette.text))
}

fn parse_inline_markdown_with_style(
    input: &str,
    palette: appearance::Palette,
    base_style: Style,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut rest = input;

    while !rest.is_empty() {
        if let Some((content, remainder)) = take_delimited(rest, "`") {
            spans.push(Span::styled(
                content.to_string(),
                Style::default()
                    .fg(palette.accent_text)
                    .bg(palette.accent_soft)
                    .add_modifier(Modifier::BOLD),
            ));
            rest = remainder;
            continue;
        }

        if let Some((content, remainder)) = take_delimited(rest, "**") {
            spans.push(Span::styled(
                content.to_string(),
                base_style.add_modifier(Modifier::BOLD),
            ));
            rest = remainder;
            continue;
        }

        if let Some((content, remainder)) = take_delimited(rest, "*") {
            spans.push(Span::styled(
                content.to_string(),
                base_style.add_modifier(Modifier::ITALIC),
            ));
            rest = remainder;
            continue;
        }

        if let Some((content, remainder)) = take_delimited(rest, "~~") {
            spans.push(Span::styled(
                content.to_string(),
                base_style.add_modifier(Modifier::CROSSED_OUT),
            ));
            rest = remainder;
            continue;
        }

        if let Some(((label, url), remainder)) = take_link(rest) {
            let visible = if label.is_empty() { url } else { label };
            spans.push(Span::styled(
                visible.to_string(),
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
            rest = remainder;
            continue;
        }

        let next = next_inline_marker(rest).unwrap_or(rest.len());
        let (segment, remainder) = rest.split_at(next);
        if !segment.is_empty() {
            spans.push(Span::styled(segment.to_string(), base_style));
        }
        if remainder.is_empty() {
            break;
        }
        spans.push(Span::styled(remainder[..1].to_string(), base_style));
        rest = &remainder[1..];
    }

    spans
}

fn next_inline_marker(input: &str) -> Option<usize> {
    ['`', '[', '*', '~']
        .into_iter()
        .filter_map(|needle| input.find(needle))
        .min()
}

fn take_delimited<'a>(input: &'a str, delimiter: &str) -> Option<(&'a str, &'a str)> {
    let stripped = input.strip_prefix(delimiter)?;
    let end = stripped.find(delimiter)?;
    Some((&stripped[..end], &stripped[end + delimiter.len()..]))
}

fn take_link(input: &str) -> Option<((&str, &str), &str)> {
    let stripped = input.strip_prefix('[')?;
    let label_end = stripped.find("](")?;
    let url_end = stripped[label_end + 2..].find(')')?;
    let label = &stripped[..label_end];
    let url = &stripped[label_end + 2..label_end + 2 + url_end];
    let remainder = &stripped[label_end + 3 + url_end..];
    Some(((label, url), remainder))
}

fn parse_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|&ch| ch == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    trimmed[level..]
        .strip_prefix(' ')
        .map(|title| (level, title))
}

fn parse_fence_start(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let stripped = trimmed
        .strip_prefix("```")
        .or_else(|| trimmed.strip_prefix("~~~"))?;
    Some(stripped.trim().to_string())
}

fn is_fence_delimiter(line: &str) -> bool {
    matches!(line.trim(), "```" | "~~~")
}

fn is_thematic_break(line: &str) -> bool {
    let compact = line
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    compact.len() >= 3 && compact.chars().all(|ch| matches!(ch, '-' | '*' | '_'))
}

fn parse_task_item(line: &str) -> Option<(bool, &str, usize)> {
    let trimmed = line.trim_start();
    let indent = line.len().saturating_sub(trimmed.len()) / 2;
    parse_prefixed_item(trimmed, ["- [ ] ", "* [ ] ", "+ [ ] "])
        .map(|body| (false, body, indent))
        .or_else(|| {
            parse_prefixed_item(
                trimmed,
                ["- [x] ", "* [x] ", "+ [x] ", "- [X] ", "* [X] ", "+ [X] "],
            )
            .map(|body| (true, body, indent))
        })
}

fn parse_unordered_item(line: &str) -> Option<(&str, usize)> {
    let trimmed = line.trim_start();
    let indent = line.len().saturating_sub(trimmed.len()) / 2;
    parse_prefixed_item(trimmed, ["- ", "* ", "+ "]).map(|body| (body, indent))
}

fn parse_ordered_item(line: &str) -> Option<(&str, &str, usize)> {
    let trimmed = line.trim_start();
    let indent = line.len().saturating_sub(trimmed.len()) / 2;
    let digits = trimmed.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits == 0 || !trimmed[digits..].starts_with(". ") {
        return None;
    }
    Some((&trimmed[..digits], &trimmed[digits + 2..], indent))
}

fn parse_prefixed_item<'a, const N: usize>(input: &'a str, prefixes: [&str; N]) -> Option<&'a str> {
    prefixes
        .into_iter()
        .find_map(|prefix| input.strip_prefix(prefix))
}
